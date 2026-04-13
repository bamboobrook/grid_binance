# Strategy Grid Gap Round 7 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining strategy-configuration and deletion gaps so stopped strategies actually disappear after deletion, every-grid custom editing works through GUI controls, reference price can use either manual input or live market price, and grid semantics match the documented spot/futures behavior more closely.

**Architecture:** Keep the existing Next.js form-to-route-to-Rust flow. Fix deletion at the strategy list boundary, extend the shared workspace form to own a real per-level editor and reference-price mode, and tighten runtime/order semantics with focused backend tests rather than redesigning the whole engine.

**Tech Stack:** Next.js App Router, React, TypeScript, Rust, Axum, PostgreSQL, Node test runner, Playwright

---

### Task 1: Lock the new failures in tests

**Files:**
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `tests/verification/strategy_surface_contract.test.mjs`
- Modify: `tests/e2e/user_app.spec.ts`

- [ ] Add a backend regression proving deleted strategies no longer appear in the user list.
- [ ] Add a UI contract proving the workspace exposes a per-level editor surface, a reference-price source selector, and a per-grid take-profit label.
- [ ] Add or extend an e2e/user flow that creates a draft in custom mode and deletes a stopped draft from the list.

### Task 2: Fix deletion semantics end-to-end

**Files:**
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/web/app/[locale]/app/strategies/page.tsx`

- [ ] Hide archived strategies from the normal user strategy list response.
- [ ] Keep delete behavior idempotent and user-readable when the strategy is already archived.
- [ ] Verify stopped strategies can still be deleted and disappear from the UI list after redirect.

### Task 3: Replace JSON-only custom editing with real every-grid controls

**Files:**
- Modify: `apps/web/components/strategies/strategy-workspace-form.tsx`
- Modify: `apps/web/components/strategies/strategy-visual-preview.tsx`
- Modify: `apps/web/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/app/api/user/strategies/[id]/route.ts`
- Modify: `docs/user-guide/create-grid-strategy.md`
- Modify: `docs/user-guide/zh/create-grid-strategy.md`

- [ ] Add a client-side per-level editor with editable entry price, spacing-to-previous grid, per-grid amount, per-grid take profit, and optional trailing take profit.
- [ ] Serialize the edited ladder back into `levels_json` so the existing backend payload shape still works.
- [ ] Rename batch take-profit copy to grid take-profit copy and default overall stop loss to empty/no-stop.
- [ ] Keep batch generation available, but let users switch into per-grid editing without hand-writing JSON.

### Task 4: Add reference-price source and verify runtime semantics

**Files:**
- Modify: `apps/web/components/strategies/strategy-workspace-form.tsx`
- Modify: `apps/web/app/api/user/strategies/create/route.ts`
- Modify: `apps/web/app/api/user/strategies/[id]/route.ts`
- Modify: `apps/trading-engine/src/strategy_runtime.rs`
- Modify: `tests/simulation/strategy_rebuild.rs`
- Modify: `tests/simulation/trailing_tp.rs`

- [ ] Add `referencePriceMode` with manual-price and current-price options.
- [ ] When current-price mode is selected, resolve the symbol price from Binance public market endpoints before batch-building levels.
- [ ] Verify trailing take profit still uses post-activation extremes and taker exits.
- [ ] Verify overall take profit and overall stop loss continue to operate on total active exposure, and document the chosen definition clearly.
- [ ] Add a focused regression for classic spot order-side behavior so the implementation matches the documented two-way spot semantics or fails loudly if inputs cannot support it.

### Task 5: Verify on the running stack

**Files:**
- Test: `node --test tests/verification/strategy_surface_contract.test.mjs`
- Test: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow`
- Test: `source "$HOME/.cargo/env" && cargo test --test trailing_tp --test strategy_rebuild`
- Test: `pnpm build:web`
- Test: `pnpm exec playwright test tests/e2e/user_app.spec.ts --grep "strategy"`

- [ ] Run the updated verification tests and confirm they fail before the fix.
- [ ] Implement the minimal fixes.
- [ ] Re-run all targeted tests and web build until green.
