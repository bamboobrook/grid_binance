# Binance Grid SaaS Commercial Recovery Design

## 1. Purpose

This document supersedes the weakened execution interpretation that produced a runnable but largely skeletal V1. It keeps the approved March 31 product scope, but redefines the implementation target as a commercially usable first release rather than a demo-grade scaffold.

The recovery target is:

- Real user-facing product UX, not placeholder pages
- Real operator-facing admin UX, not navigation stubs
- Real persistent runtime state, not in-memory service state
- Real strategy, billing, notification, and analytics workflows across restarts
- Real deployment posture for continuous operation

## 2. Current Gap Summary

The current branch provides a useful baseline, but it is not yet suitable for real users. The main gaps are:

- Frontend routes exist, but many pages are only headings plus one sentence
- User workflows are not end-to-end in the UI
- Admin workflows are mostly descriptive, not operational
- `analytics` still serves sample fills instead of persisted user data
- `exchange` symbol and credential state are not durably stored
- `telegram` bindings and inbox state are still in-memory
- Runtime architecture does not yet match the approved production direction of PostgreSQL plus Redis
- Important business entities from the spec are not yet represented as durable operational data

## 3. Recovery Decision

The recovery path will be an in-place commercial hardening of the existing `feature/full-v1` branch.

Chosen approach:

- Keep the existing Rust workspace, route topology, tests, and deployment assets
- Replace demo-grade storage and service implementations with production-grade persistence
- Replace placeholder pages with structured, task-oriented web screens
- Preserve already-correct contracts where possible to limit churn
- Expand verification from “builds and smokes” to “real user flows and operator flows”

Rejected approaches:

- Patch only the visible pages while keeping demo backend behavior
- Restart from an empty branch and throw away the working baseline

## 4. Commercial V1 Scope

### 4.1 Architecture

The release target must use:

- PostgreSQL as the primary relational store
- Redis for cache, distributed coordination, and short-lived runtime state
- Rust services for API, scheduler, trading engine, market data gateway, and chain listener
- Next.js for the public, user, help, and admin web applications
- Docker Compose for local and single-node deployment

SQLite remains acceptable only for isolated tests and developer bootstrap, not for the production path.

### 4.2 User Product Requirements

The user-facing product must provide:

- A complete public landing page with feature explanation, pricing summary, risk notices, and clear entry points
- Registration, login, email verification, password reset, and 2FA setup flows with usable forms and feedback
- A dashboard that shows membership state, exchange connectivity, strategy counts, alerts, and actionable next steps
- A membership page that lets users create payment orders, see assigned addresses, expiry, grace state, and payment instructions
- An exchange page that supports API credential save, connection test, hedge-mode requirement explanation, and symbol search
- A strategy list page with filters, batch actions, and clear status chips
- A strategy detail/workspace page with editable parameters, preflight results, start/pause/delete controls, and warnings
- An analytics page with account-level and per-strategy statistics, fills, fees, funding, and export entry points
- A notifications page with Telegram binding, in-app inbox, delivery status, and expiry reminders
- A help center with practical user guidance instead of placeholder text

### 4.3 Admin Product Requirements

The admin-facing product must provide:

- An overview dashboard with membership, deposit, strategy, and exception summaries
- User management with membership override actions and account status visibility
- Deposit/order management with exact-match results, abnormal cases, and manual handling queues
- Address pool management with pool expansion, lock visibility, and utilization status
- Template management with create, review, and apply actions
- Audit views for critical operator actions
- System configuration surfaces for plan pricing, chain/token pricing, confirmation thresholds, and treasury settings

### 4.4 Runtime and Data Requirements

The commercial recovery must add durable state for:

- Exchange accounts and masked credentials metadata
- Symbol metadata snapshots and searchable indexes
- Membership plans, billing orders, address assignments, deposit matches, and abnormal deposit records
- Telegram bindings and notification inbox items
- Strategy definitions, revisions, preflight records, runtime incidents, and stop reasons
- Exchange wallet snapshots, fills, fee records, funding records, and strategy/account analytics projections
- Admin audit logs and operational config

### 4.5 UX Requirements

The product must behave like a usable SaaS, not an API showcase.

This means:

- Every important page has app chrome, navigation, hierarchy, and content density
- Every action has visible success, failure, and loading feedback
- Every blocked action explains why it is blocked and what the user should do next
- Forms contain field grouping, helper text, warnings, and validation messaging
- Membership expiry, API readiness, and preflight failures are surfaced as actionable warnings
- Admin screens favor tables, summaries, and decision-oriented layouts over plain text paragraphs

### 4.6 Operational Requirements

The recovery must deliver:

- Compose services for PostgreSQL and Redis in addition to the existing application services
- Environment templates that document the production path clearly
- Migration support for production database bootstrap
- Seed/bootstrap helpers for admin operator setup
- Smoke verification that proves the product is reachable and that the major entry points render correctly

## 5. Acceptance Standard

The commercial recovery is complete only when all of the following are true:

- A new user can register, log in, configure security, create a payment order, save exchange credentials, create a strategy, run preflight, and understand why start succeeds or fails through the web UI
- An operator can manage memberships, deposits, address pools, templates, and audit views through the admin UI
- Restarting the stack does not lose exchange, membership, strategy, Telegram, or analytics state
- Analytics screens are driven by persisted data rather than sample fixtures
- The site reads like a product and operations console, not a test harness

## 6. Recovery Workstreams

Implementation is grouped into four workstreams:

1. Production data and service hardening
2. User product UX completion
3. Admin product UX completion
4. Deployment, observability, and acceptance hardening

## 7. Supersession Rule

This document overrides any earlier plan interpretation that treats:

- placeholder pages as sufficient delivery
- in-memory services as acceptable runtime behavior
- sample analytics as production-ready statistics
- “minimal shell” tasks as equivalent to the approved commercial V1 scope

Implementation after this point must target the commercial recovery standard defined here.
