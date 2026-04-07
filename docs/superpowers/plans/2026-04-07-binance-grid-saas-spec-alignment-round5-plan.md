# Binance Grid SaaS Spec Alignment Round 5 Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining frozen-spec gaps after the April 7 re-audit so the `full-v1` worktree stops overstating lifecycle behavior, admin security posture, and exchange-failure notifications.

**Architecture:** Keep the current Rust services and Next.js app. Do not redesign the product. Only finish the remaining truth gaps the re-audit still found: stop/resume lifecycle must reflect real exchange actions, admin TOTP requirements must be enforced without an unsafe bootstrap hole, API invalidation notifications must come from real exchange failure paths, and the final user/admin docs must match those behaviors.

**Tech Stack:** Rust, Axum, shared-db, shared-binance, trading-engine, scheduler, Next.js App Router, PostgreSQL, Redis.

---

### Task 1: Make stop/resume/delete semantics honest and exchange-backed

**Files:**
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/trading-engine/src/order_sync.rs`
- Modify: `apps/trading-engine/src/main.rs`
- Modify: `apps/api-server/tests/strategy_flow.rs`
- Modify: `docs/user-guide/manage-strategy.md`

- [ ] Add failing tests proving `stop` and `stop-all` do not claim completion before exchange cancel/close reconciliation finishes.
- [ ] Add failing tests proving `resume` rebuilds from current market inputs instead of blindly replaying saved ladder prices.
- [ ] Route stop requests through exchange-backed cancel + market-close effects, then mark terminal status only after reconciliation.
- [ ] Tighten delete visibility so archived/stopped semantics match the frozen user expectation and docs.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow && cargo test -p trading-engine`

### Task 2: Enforce admin TOTP requirement without leaving an unsafe bootstrap path

**Files:**
- Modify: `apps/api-server/src/services/auth_service.rs`
- Modify: `apps/api-server/src/routes/security.rs`
- Modify: `apps/api-server/tests/auth_flow.rs`
- Modify: `apps/api-server/tests/profile_flow.rs`
- Modify: `docs/admin-guide/operations.md`
- Modify: `docs/deployment/env-and-secrets.md`

- [ ] Add failing tests proving configured admin accounts cannot keep logging in indefinitely without completing the required TOTP setup.
- [ ] Define one explicit bootstrap rule for first-time admin TOTP enrollment that still blocks admin control-plane access until TOTP is active.
- [ ] Remove any ambiguity in docs about how operator/super-admin accounts are provisioned and how first login works.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test auth_flow --test profile_flow`

### Task 3: Emit API invalidation notifications from real exchange failure paths

**Files:**
- Modify: `apps/api-server/src/services/exchange_service.rs`
- Modify: `apps/api-server/src/services/telegram_service.rs`
- Modify: `apps/api-server/tests/notification_flow.rs`
- Modify: `apps/web/src/app/app/exchange/page.tsx`
- Modify: `docs/user-guide/getting-started.md`

- [ ] Add failing tests proving unhealthy credential refresh or validation degradation produces `ApiCredentialsInvalidated` records without calling the manual dispatch route.
- [ ] Emit the notification from the real credential save / periodic refresh path with actionable payload explaining what failed.
- [ ] Make the exchange page surface the same remediation guidance the notification uses.
- [ ] Re-run: `source "$HOME/.cargo/env" && cargo test -p api-server --test notification_flow && pnpm build:web`

### Task 4: Final docs and acceptance sweep

**Files:**
- Modify: `docs/user-guide/manage-strategy.md`
- Modify: `docs/user-guide/troubleshooting.md`
- Modify: `docs/admin-guide/operations.md`
- Modify: `tests/verification/commercial_docs_and_acceptance.test.mjs`

- [ ] Update the user/admin guides so stop semantics, admin 2FA, and exchange invalidation behavior match the real implementation.
- [ ] Re-run the targeted verification suite:
- [ ] `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow --test auth_flow --test profile_flow --test notification_flow --test analytics_flow --test membership_flow --test admin_deposit_flow`
- [ ] `source "$HOME/.cargo/env" && cargo test -p trading-engine`
- [ ] `pnpm build:web`
- [ ] `node --test tests/verification/web_app_shell.test.mjs tests/verification/commercial_docs_and_acceptance.test.mjs`
