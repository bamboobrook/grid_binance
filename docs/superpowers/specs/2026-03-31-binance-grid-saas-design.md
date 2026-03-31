# Binance Grid SaaS Design

## 1. Summary

This document defines the full V1 design for a public-facing Binance grid trading SaaS platform.

The platform targets a multi-tenant SaaS model with:

- User web app
- Admin web app
- Rust backend services
- Rust grid trading engine
- PostgreSQL and Redis
- Binance spot and futures integration
- Chain-based membership payment flow
- Telegram notifications
- Full documentation for users and administrators

The repository starts from an empty baseline. This design freezes the product, architecture, business rules, data model, lifecycle rules, and delivery scope before implementation.

## 2. Product Goals

- Build a complete public-facing Binance grid trading platform, not a single bot script
- Support Binance spot, USDⓈ-M futures, and COIN-M futures
- Use Rust for backend and trading services
- Use WebSocket market data for active strategies to maximize responsiveness
- Provide full user lifecycle: register, login, email verification, 2FA, password reset, security center
- Provide full strategy lifecycle: create, edit, save, pre-check, start, pause, resume, stop, delete
- Provide full membership lifecycle: plan selection, chain payment, automatic on-chain detection, grace period, manual admin override
- Provide full statistics: wallet balances, exchange trade history, account-level and strategy-level profit and loss, fees, funding, fills, and costs
- Provide user-facing and admin-facing documentation inside the repository and in-product help center
- Ship as a complete Docker Compose deployment

## 3. Non-Goals

- Binance options support
- Binance testnet support
- Kubernetes deployment in V1
- Automatic full RBAC customization
- Auto-restart-all strategies feature
- Platform-custodied exchange assets beyond membership collection wallets
- Dynamic template sync into already-applied user strategies

## 4. Frozen Product Decisions

### 4.1 Delivery Model

- Full V1 scope, not MVP reduction
- Multi-tenant SaaS
- One shared platform for all users
- Frontend uses TypeScript
- Backend and trading services use Rust
- Single frontend project with route partitioning
  - `/app/*` for users
  - `/admin/*` for administrators
- First release deploys with Docker Compose

### 4.2 Exchange Scope

- Binance only
- User provides their own Binance API key and secret
- Platform stores encrypted credentials
- Users must not enable withdrawal permission
- Spot supported
- USDⓈ-M futures supported
- COIN-M futures supported
- Binance options not supported
- Futures require Hedge Mode
- Futures allow isolated or cross margin
- Users can self-set leverage in futures
- Full exchange symbol support is achieved via symbol metadata sync and symbol search
- Symbol metadata sync runs every 1 hour

### 4.3 User Account and Security

- Email registration required
- Email verification required before login
- Password login enabled
- Password reset via email verification code
- TOTP 2FA is supported in V1 for users and admins
- Admin accounts must use TOTP 2FA
- Users can enable or disable TOTP 2FA from the security center
- One user can bind only one Binance account
- Admin cannot view Binance secret in plaintext
- API key is shown only in masked form

### 4.4 Membership and Billing

- Membership required before running strategies
- Default initial plan pricing
  - Monthly: 20 USD equivalent
  - Quarterly: 18 USD equivalent per month
  - Yearly: 15 USD equivalent per month
- Admin can modify plan price and duration
- Price changes affect the next renewal, not current entitlement
- Renewal stacking is allowed
- Admin can manually open, extend, freeze, unfreeze, and revoke memberships
- Membership expiry enters a 48-hour grace period
- During grace period, existing running strategies may continue
- After grace period, all running strategies are auto-paused and new starts are blocked
- User web app must show popup reminders about expiry and grace period

### 4.5 Chain Payment Rules

- Supported chains
  - Ethereum
  - BSC
  - Solana
- Supported assets
  - USDT
  - USDC
- Admin configures payable amount separately by chain and token
- Automatic on-chain monitoring is required
- Each chain starts with an address pool of 5 addresses
- Admin can expand pool size later in admin UI
- One order gets one assigned address
- Address assignment locks the address for 1 hour
- Locked address is dedicated to that order during the lock window
- If the pool is full, orders enter a queue
- Address pools are reused in rotation
- Payment amount must match the order amount exactly
- Overpayment, underpayment, wrong token, or abnormal transfer goes to admin manual handling
- Payment page must strongly warn users that amount must exactly match the order
- Chain confirmation count is configurable per chain by admin
- Admin-facing wallet sweep function is required to collect stablecoins from pool addresses into treasury wallets
- Wallet sweep actions must be audited

### 4.6 Strategy Scope

- Spot modes
  - Classic two-way spot grid
  - Buy-only spot grid
  - Sell-only spot grid
- Futures modes
  - Long grid
  - Short grid
  - Neutral two-way grid
- Spot allows multiple strategies on the same symbol
- Futures allow only one strategy per user per symbol per direction
- Futures long and short may coexist on the same symbol
- Batch actions required
  - Batch start by filtered result
  - Batch pause by filtered result
  - Batch delete by filtered result
  - Global stop-all
- Global start-all is not supported

### 4.7 Grid Configuration Rules

- Users can search symbols with fuzzy search
- Users can configure grid amount by asset quantity or USDT amount
- Users can configure per-grid amount individually
- Users can configure per-grid amount in batch
- Users can configure grid spacing in batch
- Users can configure every grid manually
- Supported generation modes
  - Arithmetic
  - Geometric
  - Fully custom
- Admin templates are supported
- Templates are copied into user-owned strategy configurations
- Template updates do not affect already-applied user strategies
- Users can freely modify template-derived strategies
- Strategy edits require pause first
- Strategy edits must be saved before restart
- Running strategy parameters cannot be hot-modified

### 4.8 Take Profit and Stop Loss Rules

- Each grid supports its own take-profit range
- Grid take-profit can be set in batch or individually
- Optional trailing take profit is supported
- Overall take profit is supported
- Optional overall stop loss is supported
- Trailing take profit behavior
  - If trailing is not set, use maker-style take-profit order placement
  - If trailing is set, do not use maker take-profit
  - Once base take-profit threshold is reached, track the highest price reached after activation
  - When price retraces by the configured trailing percentage from the post-activation high, use taker market close
  - Trailing percentage must be less than or equal to the corresponding grid take-profit range
  - The UI and pre-check must warn users that trailing take profit uses taker execution and may increase fees
- After take profit or stop loss, user can choose per strategy
  - Stop after execution
  - Rebuild and continue a new cycle

### 4.9 Lifecycle Rules

- Strategy creation begins in draft state
- Start requires full pre-flight validation
- Pre-flight must fail fast and clearly show which step failed
- Pause semantics
  - Stop strategy scheduling
  - Cancel all current working orders for that strategy
  - Keep spot holdings and futures positions
- Resume semantics
  - Run pre-flight again
  - Rebuild the grid using current saved parameters, current market, and current holdings
- Explicit stop action required
  - Cancel working orders
  - Market close relevant positions
  - Mark strategy stopped
- Delete allowed only when there are no working orders and no remaining positions
- Runtime exception semantics
  - Auto-pause the affected strategy
  - Notify via web and Telegram
  - Show clear failure reason and remediation guidance

### 4.10 Notification Rules

- Telegram notifications supported
- Bind flow
  - User generates one-time bind code in web app
  - User sends bind code to Telegram bot
- One user binds one Telegram account only
- Notification scope
  - Strategy start and pause
  - API invalidation
  - Membership reminders
  - Deposit success
  - Overall take profit or stop loss
  - Every grid fill
  - Per-fill profit information
  - Running cumulative PnL summaries tied to fills

### 4.11 Export and Audit

- CSV export required for
  - Order records
  - Fill records
  - Strategy statistics
  - Payment records
- Admin audit log is mandatory
- Audit log covers all critical admin actions

## 5. Technical Architecture

### 5.1 Top-Level Architecture

Recommended implementation model is a modular monolith plus specialized workers.

- One frontend project
- One API service
- One dedicated trading engine service
- One dedicated market data gateway service
- One dedicated chain listener service
- One scheduler service
- Shared Rust crates for domain and infrastructure

This approach is selected because the project needs:

- Strong service boundaries
- Independent runtime concerns for trading and web API
- High responsiveness on active market data
- Manageable deployment complexity in V1

### 5.2 Repository Layout

```text
apps/
  web/
  api-server/
  trading-engine/
  market-data-gateway/
  billing-chain-listener/
  scheduler/
crates/
  shared-config/
  shared-db/
  shared-auth/
  shared-binance/
  shared-chain/
  shared-events/
  shared-telemetry/
  shared-domain/
db/
  migrations/
  seeds/
deploy/
  docker/
  nginx/
  monitoring/
docs/
  superpowers/specs/
  superpowers/plans/
  user-guide/
  admin-guide/
  deployment/
tests/
  integration/
  simulation/
  e2e/
scripts/
examples/
```

### 5.3 Frontend Architecture

Frontend is a single Next.js application.

Key areas:

- Marketing site
- User app
- Admin app
- Shared component system
- Shared auth/session handling
- Shared API client layer
- Shared charting and table components
- Help center rendering from docs content

Frontend principles:

- Responsive for desktop and mobile
- Clear distinction between user operations and admin operations
- Strong validation for all trading and billing forms
- Explicit warning surfaces for risky settings
- No hidden automation on trading-critical actions

### 5.4 Backend Service Responsibilities

#### `apps/api-server`

Handles:

- Authentication and session management
- Email verification and password reset
- TOTP enable/disable and verification
- User profile and security center
- Binance credential CRUD and connection testing
- Wallet and exchange-history read APIs
- Strategy CRUD and read-side queries
- Billing order creation and read-side billing flows
- Telegram bind code generation
- Admin configuration APIs
- Reporting and export endpoints

#### `apps/trading-engine`

Handles:

- Strategy pre-flight validation
- Strategy runtime state machine
- Working order management
- Fill ingestion and attribution
- Spot and futures runtime logic
- Overall TP and SL execution
- Trailing take-profit activation and market close logic
- Runtime exception detection and auto-pause
- Strategy statistics updates

#### `apps/market-data-gateway`

Handles:

- Binance WebSocket connections
- Active-symbol subscription management
- Fan-out of market ticks to trading-engine
- Connection rotation and reconnect handling
- Stream health monitoring

Only active symbols are subscribed in real time.
Global symbol support relies on metadata sync, not permanent full-market WebSocket subscriptions.

#### `apps/billing-chain-listener`

Handles:

- Address pool assignment orchestration
- Queue handling when addresses are exhausted
- On-chain payment monitoring
- Confirmation tracking
- Abnormal deposit classification
- Membership activation events
- Sweep job execution tracking

#### `apps/scheduler`

Handles:

- Hourly Binance metadata sync
- Membership grace-period expiry checks
- Stale lock cleanup
- Retry and reconciliation jobs
- Periodic statistics compaction tasks

## 6. Binance Integration Design

### 6.1 Metadata Sync

The system must synchronize:

- Spot symbol list and filters
- USDⓈ-M symbol list and filters
- COIN-M symbol list and filters
- Trading status
- Precision filters
- Minimum order sizes
- Margin-related requirements where needed

Sync frequency:

- Full sync every 1 hour

Stored data is used for:

- User symbol search
- Pre-flight checks
- Runtime validation
- Admin monitoring

### 6.2 Credential Validation

When user adds or updates Binance credentials, the platform must validate:

- API connectivity
- Timestamp synchronization
- Trading permission enabled
- Withdrawal permission disabled
- Spot account reachability
- Futures account reachability for selected markets
- Futures Hedge Mode enabled
- Market-specific access

Credential updates require full strategy pause before applying the new credentials.

### 6.3 Futures Isolation Constraint

Because Binance futures positions are fundamentally tracked by symbol and position side, not by platform strategy instance, the system must not allow multiple same-direction futures strategies per user on the same symbol.

Allowed:

- BTCUSDT long strategy + BTCUSDT short strategy

Not allowed:

- Two BTCUSDT long strategies under the same user futures account

Spot does not have this restriction and may support multiple strategies on the same symbol.

## 7. Billing and Membership Flow Design

### 7.1 Membership Purchase Flow

1. User selects a membership plan
2. User selects chain and token
3. System creates an order using admin-configured price for that exact chain/token
4. System allocates an address or queues the order
5. If allocated, the address is locked for 1 hour
6. User pays exact amount
7. Chain listener validates transaction and confirmations
8. If valid, membership entitlement is activated or extended
9. If abnormal, order goes to admin exception handling

### 7.2 Address Pool Rules

- Pools are managed per chain
- Pools can be expanded in admin UI
- Each address can be enabled or disabled
- Assignment follows rotation
- Lock expiration after 1 hour if unpaid
- Queue must progress automatically when addresses are released

### 7.3 Membership Entitlement Rules

- Entitlements can be created automatically by payment or manually by admin
- Renewals stack on top of current expiry
- Plan changes do not rewrite active entitlements
- Next renewal uses latest configured plan prices
- Freeze and revoke are admin-controlled overrides
- Grace period lasts 48 hours after expiry

## 8. Strategy Engine Design

### 8.1 Strategy Lifecycle States

- `draft`
- `running`
- `paused`
- `error_paused`
- `completed`
- `stopped`
- `archived`

### 8.2 Strategy Revision Model

Each strategy maintains:

- Draft revision
- Active revision

Rules:

- Running strategies use active revision only
- Edits create or modify draft revision
- Resume/start moves validated draft revision into active revision
- Unsaved edits do not affect runtime

### 8.3 Pre-Flight Validation

Pre-flight must validate:

- Membership entitlement active or in grace period
- Exchange credentials valid
- Required Binance permissions enabled
- Withdrawal permission disabled
- Hedge Mode enabled for futures
- Symbol exists and is tradable
- Quantity and notional satisfy exchange filters
- Margin and leverage settings are valid
- Strategy conflict rules are satisfied
- Trailing TP configuration is valid
- Enough available balance or collateral exists

Validation output must be step-based and user-readable:

- Which step failed
- Why it failed
- What user should do next

### 8.4 Spot Runtime Logic

Spot runtime must support:

- Classic two-way grids
- Buy-only inventories
- Sell-only inventories
- Multiple same-symbol strategies per user
- Internal cost-basis tracking per strategy
- Working-order attribution per strategy and per grid level

### 8.5 Futures Runtime Logic

Futures runtime must support:

- Long grids
- Short grids
- Neutral grids
- Isolated or cross margin
- User-defined leverage
- One strategy per symbol per direction per user
- Strategy-level accounting and profit tracking
- Auto-pause on exchange or runtime anomaly

### 8.6 Take Profit and Stop Loss

#### Without trailing TP

- Use maker-style take-profit order placement
- Each grid can close according to its configured TP range

#### With trailing TP

- Base TP threshold must be reached before trailing activation
- Highest price after activation is tracked
- Retracement percentage triggers taker market close
- Retracement threshold must not exceed configured TP range

#### Overall TP and SL

- Strategy-level optional thresholds
- On trigger, close strategy according to selected post-trigger behavior
  - stop
  - rebuild

### 8.7 Strategy Termination Rules

#### Pause

- Cancel all live orders for that strategy
- Keep holdings or positions

#### Resume

- Re-run pre-flight
- Rebuild runtime grid based on saved parameters and current account state

#### Stop

- Cancel all live orders
- Market-close associated holdings or positions
- Move to `stopped`

#### Delete

- Allowed only if no live orders and no remaining holdings or positions
- Deletion is implemented as soft archive

## 9. Statistics and Reporting Design

### 9.1 User-Level Statistics

The user dashboard must show:

- Wallet balance summary
- Total realized PnL
- Total unrealized PnL
- Total fees
- Total funding fees
- Net profit
- Running strategies
- Error-paused strategies
- Recent fills
- Exchange trade history and account activity views needed for platform analytics and reconciliation
- Membership status

### 9.2 Strategy-Level Statistics

Each strategy must have independent statistics:

- Realized PnL
- Unrealized PnL
- Fees
- Funding fees
- Net profit
- Cost basis
- Fill count
- Order count
- Current state
- Current holdings or positions

### 9.3 Exports

CSV export must support:

- Orders
- Fills
- Strategy statistics
- Payment records

## 10. Notification Design

### 10.1 Telegram Bind Flow

1. User requests bind code in web app
2. System generates one-time short-lived code
3. User sends code to Telegram bot
4. System verifies and binds the Telegram identity

### 10.2 Telegram Event Types

- Strategy started
- Strategy paused
- Strategy auto-paused on error
- Fill executed
- Fill PnL details
- Overall TP or SL triggered
- Membership reminder
- Deposit confirmed
- API credential issue

## 11. Admin System Design

### 11.1 Roles

- `super_admin`
- `operator_admin`

### 11.2 Admin Capabilities

#### Super Admin

- Manage plans
- Manage address pools
- Configure confirmation counts
- Manage templates
- Manage user freezes and overrides
- Manage sweeps
- Manage system configuration
- View all reports and audits

#### Operator Admin

- View users
- View and process abnormal orders
- View reports
- View strategy runtime overview
- Operate within restricted permission boundaries

### 11.3 Audit Logging

Audit log must capture:

- Actor
- Timestamp
- Action type
- Target entity
- Before and after summary where relevant
- Source IP or session metadata where relevant

Critical actions include:

- Plan changes
- Address pool changes
- Confirmation count changes
- Template changes
- User membership overrides
- User freeze or unfreeze
- Sweep operations
- Manual abnormal-order decisions

## 12. Frontend Page Map

### 12.1 Public Pages

- `/`
- `/login`
- `/register`

### 12.2 User App Pages

- `/app/dashboard`
- `/app/exchange`
- `/app/strategies`
- `/app/strategies/new`
- `/app/strategies/:id`
- `/app/orders`
- `/app/billing`
- `/app/telegram`
- `/app/security`
- `/app/help`

### 12.3 Admin App Pages

- `/admin/dashboard`
- `/admin/users`
- `/admin/memberships`
- `/admin/deposits`
- `/admin/address-pools`
- `/admin/templates`
- `/admin/strategies`
- `/admin/sweeps`
- `/admin/audit`
- `/admin/system`

## 13. API Domain Boundaries

### 13.1 Public/User Domains

- `auth`
- `profile`
- `exchange`
- `strategies`
- `orders`
- `analytics`
- `billing`
- `telegram`

### 13.2 Admin Domains

- `admin/users`
- `admin/memberships`
- `admin/deposits`
- `admin/address-pools`
- `admin/templates`
- `admin/strategies`
- `admin/sweeps`
- `admin/audit`
- `admin/system`

## 14. Data Model

The following logical tables are required.

### 14.1 Identity and Security

- `users`
- `admin_users`
- `user_sessions`
- `email_verification_tokens`
- `password_reset_tokens`
- `user_totp_factors`
- `user_exchange_accounts`
- `user_exchange_credentials`
- `telegram_bindings`

### 14.2 Membership and Billing

- `membership_plans`
- `membership_plan_prices`
- `membership_orders`
- `membership_entitlements`
- `deposit_address_pool`
- `deposit_address_allocations`
- `deposit_transactions`
- `deposit_order_queue`
- `fund_sweep_jobs`
- `fund_sweep_transfers`

### 14.3 Trading

- `strategies`
- `strategy_revisions`
- `strategy_grid_levels`
- `strategy_runtime_positions`
- `strategy_orders`
- `strategy_fills`
- `strategy_events`
- `strategy_profit_snapshots`
- `account_profit_snapshots`
- `exchange_wallet_snapshots`
- `exchange_account_trade_history`

### 14.4 Administration and Content

- `strategy_templates`
- `system_configs`
- `audit_logs`
- `notification_logs`

## 15. Error Handling Design

The platform must prefer explicit failure over silent fallback.

Examples:

- If pre-flight fails, do not allow start
- If credential validation fails, do not save as healthy
- If address assignment is unavailable, queue the order instead of guessing
- If chain transfer is abnormal, do not auto-credit
- If runtime order management becomes inconsistent, auto-pause the strategy
- If WebSocket reconnect exceeds thresholds, raise alerts

User-visible failures must include:

- concise title
- actionable explanation
- exact failed step when applicable

## 16. Testing Strategy

### 16.1 Backend Unit Tests

Must cover:

- Grid generation
- Grid override validation
- TP and trailing TP calculations
- SL calculations
- Membership entitlement logic
- Address pool allocation and release
- Queue progression
- Amount matching
- Strategy state transitions

### 16.2 Integration Tests

Must cover:

- Binance credential check flows
- Strategy pre-flight flows
- Order and fill ingestion
- Runtime exception auto-pause
- Billing order creation and confirmation
- Admin abnormal-order handling
- Template application

### 16.3 Simulation Tests

Must cover:

- Strategy behavior under synthetic market streams
- Trailing TP retracement
- Overall TP and SL triggers
- Resume-after-pause rebuild behavior

### 16.4 Frontend E2E Tests

Must cover:

- Register and verify email
- Login with 2FA
- Bind exchange credentials
- Create and start strategy
- Billing order creation
- Telegram bind
- Admin manual membership actions

## 17. Monitoring and Operations

### 17.1 Logging

- Structured JSON logs across all services
- Shared correlation IDs
- Business identifiers embedded where possible

### 17.2 Metrics

At minimum:

- Active strategies
- Error-paused strategies
- Market ticks processed
- Order placement success rate
- Cancel success rate
- WebSocket reconnect count
- Deposit match success rate
- Address pool occupancy
- Sweep success rate

### 17.3 Alerts

At minimum:

- Binance market stream instability
- Elevated order failures
- Chain listener outage
- Address pool exhaustion
- Membership expiry job failure
- Database connectivity issues

## 18. Documentation Deliverables

### 18.1 User Guide

Required files:

- `docs/user-guide/getting-started.md`
- `docs/user-guide/binance-api-setup.md`
- `docs/user-guide/membership-and-payment.md`
- `docs/user-guide/create-grid-strategy.md`
- `docs/user-guide/manage-strategy.md`
- `docs/user-guide/security-center.md`
- `docs/user-guide/telegram-notifications.md`
- `docs/user-guide/troubleshooting.md`

### 18.2 Admin Guide

Required files:

- `docs/admin-guide/address-pool-management.md`
- `docs/admin-guide/membership-operations.md`
- `docs/admin-guide/template-management.md`
- `docs/admin-guide/abnormal-order-handling.md`
- `docs/admin-guide/system-config-and-audit.md`

### 18.3 Deployment Guide

Required files:

- `docs/deployment/docker-compose.md`
- `docs/deployment/env-and-secrets.md`
- `docs/deployment/backup-and-restore.md`

### 18.4 In-App Help

The web app help center must surface the same user documentation content from the repository.

## 19. Implementation Decomposition Note

This document is the umbrella product and architecture specification for the full project.

Implementation must still be decomposed into multiple execution plans and delivery batches, at minimum covering:

- foundation and shared infrastructure
- auth, security, and membership
- Binance integration and trading engine
- user web app and admin app
- observability, testing, docs, and release hardening

## 20. Source References

Official Binance references used to validate architecture constraints:

- Binance Spot general endpoints and exchange metadata:
  https://developers.binance.com/docs/binance-spot-api-docs/rest-api/general-endpoints
- Binance Spot WebSocket streams:
  https://developers.binance.com/docs/binance-spot-api-docs/web-socket-streams
- Binance USDⓈ-M exchange information:
  https://developers.binance.com/docs/derivatives/usds-margined-futures/market-data/rest-api/Exchange-Information
- Binance USDⓈ-M WebSocket market streams:
  https://developers.binance.com/docs/derivatives/usds-margined-futures/websocket-market-streams
- Binance COIN-M WebSocket market streams:
  https://developers.binance.com/docs/derivatives/coin-margined-futures/websocket-market-streams
- Binance USDⓈ-M position mode and position information:
  https://developers.binance.com/docs/derivatives/usds-margined-futures/account/rest-api/Get-Current-Position-Mode
  https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/Position-Information-V2
- Binance USDⓈ-M new order endpoint:
  https://developers.binance.com/docs/derivatives/usds-margined-futures/trade/rest-api/New-Order

Market references used only as product-shaping input:

- Pionex grid bot:
  https://support.pionex.com/hc/en-us/articles/45085712163225-Grid-Trading-Bot
- Pionex futures grid bot:
  https://support.pionex.com/hc/en-us/articles/45343668185113-Futures-Grid-Bot
- 3Commas grid bot:
  https://3commas.io/grid-bot
- 3Commas help references:
  https://help.3commas.io/en/articles/7931795-grid-bot-choosing-a-strategy-or-a-trading-pair

## 21. Acceptance Baseline

The design is considered complete when implementation delivers:

- Complete multi-tenant SaaS flow
- Complete user auth and security flow
- Complete membership payment and grace-period flow
- Complete Binance credential flow
- Complete strategy configuration and lifecycle flow
- Spot and futures runtime support within frozen constraints
- Telegram binding and notification flow
- Strategy-level and account-level statistics
- CSV exports
- Admin management and audit logging
- User and admin documentation
- Docker Compose deployment and operational guidance
