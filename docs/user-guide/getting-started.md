# User Guide: Getting Started

## What This Product Includes

Grid Binance V1 is a hosted Binance grid trading platform with:

- public registration and login
- a user workspace under `/app/*`
- an admin workspace under `/admin/*`
- Binance spot, USDⓈ-M futures, and COIN-M futures support
- membership billing by chain payment order
- Telegram notifications
- repository-backed help center articles under `/help/*`

The first release is deployed with Docker Compose behind Nginx and is exposed locally at `http://localhost:8080`.

## First Run Path

1. Copy `.env.example` to `.env`.
2. Set the release-critical values described in `docs/deployment/env-and-secrets.md`.
3. Start the stack with `docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build`.
4. Open `http://localhost:8080`.
5. Use the public entry points:
   - `/register` to create an account
   - `/login` to sign in
   - `/help/getting-started` to read the in-app help center from repository docs

## After Sign-In

After login, the commercial runtime path is the user app under `/app/*`.

Key user routes:

- `/app/dashboard` for account overview and renewal reminders
- `/app/billing` for membership plans, renewal orders, and payment instructions
- `/app/security` for password and TOTP operations
- `/app/strategies` for draft strategy management
- `/app/analytics` for account and strategy reporting
- `/app/notifications` for in-app alert review

Anonymous requests to `/app/*` and `/admin/*` are expected to redirect to `/login`.

## Product Rules To Remember

- Membership is required before starting a strategy.
- Each billing order must be paid with the exact chain, token, and amount shown on the billing page.
- Spot, USDⓈ-M futures, and COIN-M futures are supported.
- Futures strategies require Binance Hedge Mode.
- Withdrawal permission must stay disabled on Binance API keys.
- Help center articles are sourced from `docs/user-guide/*` in this repository.

## Non-Compose Local Note

If you are verifying the API service outside Docker Compose, override the runtime hosts and use `cargo run -p api-server` with host-reachable database and Redis values.

## Local Verification

After the stack is up, run `./scripts/smoke.sh`.

The smoke script verifies the release path through Nginx, including:

- `http://localhost:8080/`
- `http://localhost:8080/api/healthz`
- `http://localhost:8080/help/getting-started`
- `http://localhost:8080/app/dashboard`
- `http://localhost:8080/admin/dashboard`

The `/app/*` and `/admin/*` checks are expected to prove routing and auth gates, even when they redirect to login for anonymous access.
