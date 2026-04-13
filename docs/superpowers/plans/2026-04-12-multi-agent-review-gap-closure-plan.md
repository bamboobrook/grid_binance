# Multi-Agent Review Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the full set of defects found by the April 12 multi-agent review so the product aligns more closely with the frozen Binance Grid SaaS spec across strategy UX safety, dashboard/admin surface truth, notification/i18n completeness, and live trading/runtime correctness.

**Architecture:** Keep the current Rust workspace + Next.js structure. Do not redesign the product. Repair the specific truth gaps: unsafe strategy actions, missing notification surface, placeholder shell data, incomplete dashboard/admin flows, and runtime paths that currently diverge from real exchange behavior.

**Tech Stack:** Rust workspace, Axum, Tokio, SQLx/shared-db, Next.js App Router, TypeScript, Playwright, Node verification tests.

---

### Task 1: Strategy Workspace Safety And State-Machine Alignment

**Files:**
- Modify: `apps/web/app/[locale]/app/strategies/page.tsx`
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- Modify: `apps/web/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/components/strategies/strategy-workspace-form.tsx`
- Modify: `tests/e2e/user_app.spec.ts`
- Modify: `tests/e2e/user_commercial_flows.spec.ts`
- Modify: `tests/verification/strategy_surface_contract.test.mjs`

- [ ] Step 1: Add failing coverage for explicit strategy selection, illegal action hiding, running-state save restrictions, and required symbol selection.
- [ ] Step 2: Run `node --test tests/verification/strategy_surface_contract.test.mjs && pnpm exec playwright test tests/e2e/user_app.spec.ts tests/e2e/user_commercial_flows.spec.ts --grep "strategy"`.
- [ ] Step 3: Change list-page batch actions to operate only on explicit user-selected strategy ids instead of the full filtered result set.
- [ ] Step 4: Change detail/list action rendering so only backend-legal actions are shown for each status, and disable editing/saving while a strategy is running.
- [ ] Step 5: Remove the `BTCUSDT` fallback and require a symbol chosen from search results before draft save/start flows can proceed.
- [ ] Step 6: Re-run the strategy verification and e2e commands until they pass.

### Task 2: User Surface Truth, Notifications, Theme, And I18n Cleanup

**Files:**
- Modify: `apps/web/app/[locale]/app/dashboard/page.tsx`
- Modify: `apps/web/app/[locale]/app/exchange/page.tsx`
- Create: `apps/web/app/[locale]/app/notifications/page.tsx`
- Modify: `apps/web/app/[locale]/app/telegram/page.tsx`
- Modify: `apps/web/components/shell/app-shell-section.tsx`
- Modify: `apps/web/components/ui/dialog.tsx`
- Modify: `apps/web/components/strategies/strategy-visual-preview.tsx`
- Modify: `apps/web/lib/api/mock-data.ts`
- Modify: `apps/web/lib/api/server.ts`
- Modify: `tests/verification/web_app_shell.test.mjs`
- Modify: `tests/verification/web_shell_surface_contract.test.mjs`
- Modify: `tests/verification/web_user_pages_i18n.test.mjs`
- Modify: `tests/e2e/user_app.spec.ts`

- [ ] Step 1: Add failing verification coverage for a standalone notifications page, locale-clean user detail copy, theme-safe headings/dialog defaults, dashboard required stats, and removal of production placeholder identity data.
- [ ] Step 2: Run `node --test tests/verification/web_app_shell.test.mjs tests/verification/web_shell_surface_contract.test.mjs tests/verification/web_user_pages_i18n.test.mjs`.
- [ ] Step 3: Replace hard-coded shell/demo fallback content with neutral empty-state data, add `/app/notifications` as an in-app inbox page, and make the Telegram page focus on bind/delivery instead of pretending to be the only notification surface.
- [ ] Step 4: Repair dashboard and exchange summaries so they reflect actual membership/statistics/account truth instead of misleading defaults.
- [ ] Step 5: Make shared UI text fully locale-aware and theme-safe, including section headings, dialog defaults, and strategy detail copy.
- [ ] Step 6: Re-run the user-surface verification tests and relevant user E2E flows.

### Task 3: Admin Deposit/Membership Surface Contract Repair

**Files:**
- Modify: `apps/web/app/[locale]/admin/deposits/page.tsx`
- Modify: `apps/web/app/[locale]/admin/memberships/page.tsx`
- Modify: `apps/api-server/src/services/membership_service.rs`
- Modify: `apps/api-server/tests/admin_deposit_flow.rs`
- Modify: `apps/api-server/tests/membership_flow.rs`
- Modify: `tests/e2e/admin_commercial_flows.spec.ts`

- [ ] Step 1: Add failing coverage for the manual-credit confirmation phrase, abnormal-deposit reason mapping, and plan-specific membership editing UI behavior.
- [ ] Step 2: Run `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_deposit_flow --test membership_flow && pnpm exec playwright test tests/e2e/admin_commercial_flows.spec.ts`.
- [ ] Step 3: Align the admin manual-credit confirmation contract between frontend and backend, and translate all active abnormal-deposit reason codes into operator-readable text.
- [ ] Step 4: Make the admin membership editor bind clearly to the selected plan instead of silently defaulting to monthly-only editing semantics.
- [ ] Step 5: Re-run the admin Rust tests and admin commercial E2E flow.

### Task 4: Trading Runtime Stop/Pause/Resume Correctness

**Files:**
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `apps/trading-engine/src/order_sync.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/scheduler/src/jobs/membership_grace.rs`
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/trading-engine/tests/order_sync.rs`
- Modify: `apps/trading-engine/tests/execution_sync.rs`
- Modify: `tests/simulation/strategy_rebuild.rs`

- [ ] Step 1: Add failing Rust coverage for TP/SL-triggered close orders, grace-expiry pause cancel behavior, and resume/rebuild using current market plus current holdings.
- [ ] Step 2: Run `source "$HOME/.cargo/env" && cargo test -p trading-engine --test order_sync --test execution_sync && cargo test --test strategy_rebuild`.
- [ ] Step 3: Change runtime TP/SL/trailing close paths so they create real stopping/close intents instead of only mutating local positions.
- [ ] Step 4: Ensure pause and grace-expiry flows cancel all live exchange orders that were already placed, not only local working placeholders.
- [ ] Step 5: Rebuild resume/restart logic from current account state and live market reference rather than the saved midpoint shortcut.
- [ ] Step 6: Re-run the Rust runtime test set until green.

### Task 5: Partial Fill, Position Attribution, Supported Mode, And Statistics Accuracy

**Files:**
- Modify: `apps/trading-engine/src/execution_sync.rs`
- Modify: `apps/trading-engine/src/trade_sync.rs`
- Modify: `apps/trading-engine/src/runtime.rs`
- Modify: `apps/trading-engine/src/statistics.rs`
- Modify: `apps/api-server/src/services/analytics_service.rs`
- Modify: `apps/trading-engine/tests/execution_sync.rs`
- Modify: `apps/trading-engine/tests/trade_sync.rs`
- Modify: `apps/trading-engine/tests/grid_runtime.rs`
- Modify: `apps/api-server/tests/analytics_flow.rs`

- [ ] Step 1: Add failing tests for partial fills, correct position attribution by level/order, supported required modes, and strategy/user analytics matching the updated runtime truth.
- [ ] Step 2: Run `source "$HOME/.cargo/env" && cargo test -p trading-engine --test execution_sync --test trade_sync --test grid_runtime && cargo test -p api-server --test analytics_flow`.
- [ ] Step 3: Make execution/trade sync advance on partial fills, attribute closes to the correct level/order, and remove the stale path that still rejects required spec modes.
- [ ] Step 4: Update statistics aggregation so fills, fees, positions, and strategy/user summaries stay consistent with the repaired runtime behavior.
- [ ] Step 5: Re-run the targeted Rust/API analytics suites until green.

### Task 6: Final Full Verification

**Files:**
- Modify as needed: `tests/verification/*.test.mjs`
- Modify as needed: `tests/e2e/*.spec.ts`

- [ ] Step 1: Run `pnpm test`.
- [ ] Step 2: Run `pnpm build:web`.
- [ ] Step 3: Run `pnpm test:e2e`.
- [ ] Step 4: If any failure appears, fix only the real product/test mismatch uncovered in this round and repeat the full verification set.

## Spec Coverage

- Strategy scope, edits, start/pause/delete, batch action semantics, symbol selection, and stop/rebuild behavior: Tasks 1, 4, 5
- Notification rules, in-app visibility, multilingual surfaces, and theme/readability: Task 2
- Dashboard statistics and membership/account visibility: Task 2 and Task 5
- Admin abnormal-order handling, address/membership operating surfaces: Task 3
- Runtime correctness for TP/SL/trailing, membership expiry pause, partial fill sync, and required trading modes: Tasks 4 and 5
- Full regression protection and acceptance validation: Task 6
