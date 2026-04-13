# Strategy Workspace Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the user-reported strategy workspace gaps so symbol search becomes a real dropdown selector, spot/futures fields behave correctly, preview panels show useful content, strategy lifecycle actions stop failing silently, and orders/analytics surfaces reflect per-strategy truth.

**Architecture:** Keep the current Next.js plus Rust service split. Do not redesign the whole product. Replace the current datalist-based strategy picker with a client-side composer/workspace control backed by the existing symbol search API, tighten the web-to-API lifecycle route handling to expose correct states and errors, and verify the analytics/order pages against persisted backend truth instead of assuming the current UI is correct.

**Tech Stack:** Next.js App Router, React, next-intl, Rust, Axum, PostgreSQL, Playwright, Node test runner

---

### Task 1: Lock the failing strategy-composer and lifecycle behaviors in tests

**Files:**
- Modify: `tests/verification/strategy_surface_contract.test.mjs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Test: `tests/verification/strategy_surface_contract.test.mjs`
- Test: `cargo test -p api-server --test strategy_flow`

- [ ] **Step 1: Write the failing UI contract assertions**

Add assertions that the strategy creation surface no longer relies on `datalist`, exposes a dedicated symbol selector container, and keeps market-specific futures fields separate from spot-only flows.

- [ ] **Step 2: Run the UI contract test to verify it fails**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && node --test tests/verification/strategy_surface_contract.test.mjs`

Expected: FAIL because the current page still uses `datalist`, does not expose a dedicated dropdown selector, and keeps margin/leverage fields unconditional.

- [ ] **Step 3: Write failing backend lifecycle tests**

Add or extend API tests that prove:
- Draft strategies can be deleted directly when no orders or positions exist.
- Draft strategies do not pretend that pause/stop succeeded.
- Lifecycle errors remain user-readable instead of collapsing into a generic failure.

- [ ] **Step 4: Run the backend test to verify it fails**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`

Expected: FAIL on the newly added lifecycle assertions before implementation.

### Task 2: Rebuild the strategy creation and detail workspace controls

**Files:**
- Create: `apps/web/components/strategies/strategy-symbol-picker.tsx`
- Create: `apps/web/components/strategies/strategy-visual-preview.tsx`
- Create: `apps/web/components/strategies/strategy-workspace-form.tsx`
- Modify: `apps/web/app/[locale]/app/strategies/new/page.tsx`
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- Modify: `apps/web/messages/zh.json`
- Modify: `apps/web/messages/en.json`
- Test: `tests/verification/strategy_surface_contract.test.mjs`

- [ ] **Step 1: Build a real symbol-picker component**

Implement a client component that:
- receives initial search results from the server page
- renders a visible result list or dropdown
- lets the user click a row to fill the hidden `symbol` form field
- shows market/base/quote labels instead of raw datalist suggestions

- [ ] **Step 2: Make market type drive field visibility**

Move strategy form behavior into a reusable workspace form so:
- `spot` hides futures margin mode and leverage inputs
- `usd-m` and `coin-m` show futures-only controls
- mode options stay valid for the chosen market
- the selected symbol and market remain in sync

- [ ] **Step 3: Replace the empty preview panel**

Add a lightweight preview that shows at minimum:
- selected symbol and market
- reference price, grid count, spacing, amount mode
- estimated grid ladder preview derived from current inputs
- a chart/embed area or actionable placeholder that reflects the selected pair instead of a dead static panel

- [ ] **Step 4: Run the UI contract test again**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && node --test tests/verification/strategy_surface_contract.test.mjs`

Expected: PASS

### Task 3: Fix web-to-API lifecycle routing and user-visible strategy errors

**Files:**
- Modify: `apps/web/app/api/user/strategies/[id]/route.ts`
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- Test: `cargo test -p api-server --test strategy_flow`

- [ ] **Step 1: Align lifecycle semantics with actual state**

Implement route handling so:
- `pause` only targets running strategies and returns a clear message when the draft was never started
- `stop` only targets running/paused strategies and explains why a draft cannot stop
- `delete` succeeds for deletable draft/stopped strategies and shows the real blocking reason otherwise
- `start` and `resume` preserve preflight failure step and guidance

- [ ] **Step 2: Surface actionable error copy in the workspace**

Ensure the detail page shows:
- which action failed
- why it failed
- which preflight step blocked the action when applicable
- separate wording for “not started yet”, “needs pause before edit”, and “cannot delete while positions/orders remain”

- [ ] **Step 3: Run backend regression tests**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`

Expected: PASS

### Task 4: Re-audit orders and analytics surfaces against backend truth

**Files:**
- Modify: `apps/web/app/[locale]/app/orders/page.tsx`
- Modify: `apps/web/app/[locale]/app/analytics/page.tsx`
- Modify: `apps/api-server/tests/analytics_flow.rs`
- Test: `cargo test -p api-server --test analytics_flow`

- [ ] **Step 1: Verify per-strategy statistics fields are all wired from backend responses**

Check and fix the page mapping for:
- realized PnL
- unrealized PnL
- fees paid
- funding total
- net PnL
- order count, fill count, and position quantity

- [ ] **Step 2: Verify orders/history pages use real strategy/runtime data**

Check and fix the page mapping for:
- order status
- fill rows
- export labels
- strategy linkage

- [ ] **Step 3: Run analytics regression tests**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && source "$HOME/.cargo/env" && cargo test -p api-server --test analytics_flow`

Expected: PASS

### Task 5: Real acceptance on the running compose stack

**Files:**
- Modify: `tests/e2e/user_app.spec.ts`
- Test: `tests/e2e/user_app.spec.ts`

- [ ] **Step 1: Add or update a browser path that exercises strategy creation UX**

Cover:
- search symbol
- click-select symbol from dropdown
- switch `spot` and confirm futures-only fields disappear
- save draft
- open detail page
- verify delete works on a fresh draft

- [ ] **Step 2: Run the focused browser verification**

Run: `cd /home/bumblebee/Project/grid_binance/.worktrees/full-v1 && pnpm exec playwright test tests/e2e/user_app.spec.ts --grep "strategy"`

Expected: PASS

- [ ] **Step 3: Manually verify against the running compose stack**

Run:
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml ps`
- `curl -I http://127.0.0.1:8080`

Expected:
- all required services healthy
- HTTP entrypoint reachable before closing the task
