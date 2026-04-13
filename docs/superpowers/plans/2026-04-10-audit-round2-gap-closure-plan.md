# Audit Round 2 Gap Closure Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the remaining real-world correctness gaps found in the second full audit so trading lifecycle, statistics truth, and visible user/admin surfaces match the frozen requirements more closely.

**Architecture:** Keep the current Rust API + Rust trading engine + Next.js structure. Do not redesign the product. Fix the remaining truth gaps in place: REST trade backfill must stop pretending partial fills are fully filled, strategy snapshots must stop proportionally allocating account-wide unrealized/funding as if they were independent strategy truth, frontend strategy analytics must expose the new long/short breakdown fields, and dead or misleading auth/deposit helper branches must be reduced where safe.

**Tech Stack:** Rust, Axum, Next.js App Router, TypeScript, PostgreSQL, Redis

---

## P0
- Fix `apps/trading-engine/src/trade_sync.rs` so REST trade backfill does not fabricate `FILLED` semantics per individual trade record.
- Add regression tests in `apps/trading-engine/tests/trade_sync.rs` for multi-trade same-order backfill.
- Rework `apps/scheduler/src/main.rs` strategy snapshot sync so per-strategy unrealized/funding is not a naive proportional share of account totals.
- Add snapshot regressions in `apps/scheduler/src/main.rs` tests for multi-strategy same-account allocation.
- Update `apps/web/app/[locale]/app/analytics/page.tsx` and `apps/web/app/[locale]/app/strategies/[id]/page.tsx` to show long/short breakdown fields when present.

## P1
- Quantize runtime-derived exit orders again before exchange submission in `apps/trading-engine/src/order_sync.rs` using stored symbol filter metadata.
- Add coverage for TP/restore orders on symbols with non-trivial tick and step sizes.
- Remove duplicate placeholder pool condition in `apps/api-server/src/services/membership_service.rs` and tighten non-production seeding guard.

## P2
- Clean up dead verify-email user flow surfaces so registration/login UX matches the current “register then login directly” behavior.
- Add graceful admin shell fallback in `apps/web/lib/api/admin-product-state.ts` when one supporting admin endpoint fails.
