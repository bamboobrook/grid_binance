# Web Auth Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the last web auth blocker by wiring the same session secret into compose web, enabling real browser register/login flows, and replacing forged-cookie e2e paths with real sessions.

**Architecture:** Add minimal web-side auth route handlers that proxy to the existing backend auth API, set the `session_token` cookie, and redirect back into the existing proxy-guarded app/admin routes. Keep the current proxy verification scheme and make compose plus Playwright use the same backend secret/base URL as the API server.

**Tech Stack:** Next.js App Router, route handlers, Playwright, Node test runner, Docker Compose, Rust API server

---

### Task 1: Lock failing auth/compose expectations

**Files:**
- Modify: `tests/verification/compose.test.mjs`
- Modify: `tests/e2e/user_app.spec.ts`
- Modify: `tests/e2e/admin_app.spec.ts`
- Create: `tests/e2e/support/auth.ts`
- Modify: `apps/web/playwright.config.ts`

- [ ] **Step 1: Write failing compose and e2e assertions**
- [ ] **Step 2: Run targeted verification/e2e to confirm failures**

### Task 2: Implement minimal web auth bridge

**Files:**
- Create: `apps/web/src/lib/auth.ts`
- Create: `apps/web/src/app/api/auth/login/route.ts`
- Create: `apps/web/src/app/api/auth/register/route.ts`
- Modify: `apps/web/src/app/(public)/login/page.tsx`
- Modify: `apps/web/src/app/(public)/register/page.tsx`
- Modify: `deploy/docker/docker-compose.yml`

- [ ] **Step 1: Add shared backend auth helper and cookie writer**
- [ ] **Step 2: Implement login/register route handlers with backend proxying**
- [ ] **Step 3: Replace static shells with real forms and error rendering**
- [ ] **Step 4: Wire compose web env to the same auth secret and backend base URL**

### Task 3: Verify and commit

**Files:**
- Modify: `tests/e2e/support/sessionToken.ts` (delete if unused)

- [ ] **Step 1: Run `pnpm build`**
- [ ] **Step 2: Run `node --test tests/verification/*.test.mjs`**
- [ ] **Step 3: Run `pnpm test:e2e`**
- [ ] **Step 4: Remove dead forged-cookie helper if unused**
- [ ] **Step 5: Commit with message including 问题描述/修复思路/复现路径**
