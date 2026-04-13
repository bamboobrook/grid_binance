# Binance Grid SaaS Final Gap Round 6 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the final high-impact user-visible and rule-enforcement gaps found in the April 9 audit so locale routing, strategy workspace UX, admin TOTP policy, and sweep audit behavior align with the frozen design plus later user decisions.

**Architecture:** Keep the current Next.js plus Rust multi-service layout. Do not redesign features. This round only removes broken locale redirects/routes, finishes the strategy detail editing surface in bilingual form, enforces the admin TOTP invariant, and records auditable sweep state transitions. Registration stays email-code-free because the user explicitly changed that product rule after the original design freeze.

**Tech Stack:** Next.js App Router, TypeScript, Rust, Axum, shared-db, PostgreSQL.

---

### Task 1: Fix locale-safe page routing and form redirects

**Files:**
- Modify: `apps/web/app/[locale]/app/membership/page.tsx`
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- Modify: `apps/web/app/[locale]/admin/strategies/page.tsx`
- Modify: `apps/web/app/[locale]/admin/templates/page.tsx`
- Modify: `apps/web/app/api/user/exchange/route.ts`
- Modify: `apps/web/app/api/user/security/route.ts`
- Modify: `apps/web/app/api/user/telegram/route.ts`
- Modify: `apps/web/app/api/user/strategies/batch/route.ts`
- Modify: `apps/web/app/api/user/strategies/templates/route.ts`
- Test: `tests/verification/web_route_locale_redirects.test.mjs`
- Test: `tests/verification/web_user_navigation_contract.test.mjs`

- [ ] Replace all bare `/app/*`, `/admin/*`, and `/login` redirects in the touched files with `localizedAppPath`, `localizedAdminPath`, `localizedPublicPath`, or locale-aware page href/form actions.
- [ ] Ensure route-localized pages keep the active locale in link clicks and GET form submissions.
- [ ] Run `node --test tests/verification/web_route_locale_redirects.test.mjs tests/verification/web_user_navigation_contract.test.mjs`.

### Task 2: Finish strategy detail page bilingual and lifecycle UI copy

**Files:**
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- Test: `tests/verification/strategy_surface_contract.test.mjs`
- Test: `tests/verification/web_user_pages_i18n.test.mjs`

- [ ] Add locale-aware copy for every remaining visible control, banner, metric, table, warning, and error on strategy detail.
- [ ] Keep current backend contract intact while making the page clearly explain pause-before-edit, save-before-restart, pre-flight failure reasons, and trailing take-profit taker fee warning.
- [ ] Run `node --test tests/verification/strategy_surface_contract.test.mjs tests/verification/web_user_pages_i18n.test.mjs`.

### Task 3: Enforce admin TOTP invariant across security routes

**Files:**
- Modify: `apps/api-server/src/services/auth_service.rs`
- Modify: `apps/api-server/tests/auth_flow.rs`
- Modify: `apps/api-server/tests/profile_flow.rs`

- [ ] Add failing coverage proving configured admin accounts cannot disable TOTP through the shared security endpoint.
- [ ] Make admin-role users return a clear forbidden error when attempting to disable TOTP.
- [ ] Re-run `source "$HOME/.cargo/env" && cargo test -p api-server --test auth_flow --test profile_flow`.

### Task 4: Audit sweep submission and terminal transitions

**Files:**
- Modify: `apps/billing-chain-listener/src/main.rs`
- Modify: `apps/api-server/tests/admin_deposit_flow.rs`
- Modify: `apps/api-server/tests/admin_address_pools_flow.rs`
- Modify: `crates/shared-db/src/postgres/billing.rs`
- Modify: `crates/shared-db/src/lib.rs`

- [ ] Add failing tests that require audit entries for sweep submission and terminal failure/success state changes.
- [ ] Record audit log rows whenever a sweep transfer/job moves into submitted, confirmed, or failed lifecycle states.
- [ ] Re-run `source "$HOME/.cargo/env" && cargo test -p api-server --test admin_deposit_flow --test admin_address_pools_flow && cargo test -p billing-chain-listener`.

### Final Acceptance

- [ ] `source "$HOME/.cargo/env" && cargo test -p api-server --test auth_flow --test profile_flow --test admin_deposit_flow --test admin_address_pools_flow`
- [ ] `source "$HOME/.cargo/env" && cargo test -p billing-chain-listener`
- [ ] `pnpm --dir apps/web build`
- [ ] `node --test tests/verification/web_route_locale_redirects.test.mjs tests/verification/web_user_navigation_contract.test.mjs tests/verification/strategy_surface_contract.test.mjs tests/verification/web_user_pages_i18n.test.mjs`
