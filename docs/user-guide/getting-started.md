# User Guide: Getting Started

## What This Product Includes

Grid Binance V1 is a hosted Binance grid trading platform with:

- public registration and login
- a user workspace under `/app/*`
- an admin workspace under `/admin/*`
- Binance spot, USDⓈ-M futures, and COIN-M futures support
- membership billing by chain payment order
- Telegram notifications
- repository-backed help articles rendered in both the in-app help center and the public help route

The first release is deployed with Docker Compose behind Nginx and is exposed locally at `http://localhost:8080`.

## First Run Path

1. Copy `.env.example` to `.env`.
2. Set the release-critical values described in `docs/deployment/env-and-secrets.md`.
3. Start the stack with `docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build`.
4. Open `http://localhost:8080`.
5. Use the public entry points:
   - `/register` to create an account
   - `/login` to sign in
   - `/help/getting-started` as the public help route for the same repository-backed article

## After Sign-In

After login, the commercial runtime path is the user app under `/app/*`.

Key user routes:

- `/app/dashboard` for account overview and renewal reminders
- `/app/billing` for membership plans, renewal orders, and payment instructions
- `/app/security` for password and TOTP operations
- `/app/exchange` for Binance API credential save, masking, and connection tests
- `/app/strategies` for draft strategy management
- `/app/strategies/new` for creating a new draft strategy
- `/app/orders` for fills, order history, and account activity review
- `/app/telegram` for Telegram bot binding and notification delivery status
- `/app/help?article=getting-started` for the in-app help center view of this article

Use `/app/help?article=<slug>` when reading help inside the authenticated app shell. Use `/help/<slug>` when sharing the same article on the public help route without the app shell.

Anonymous requests to `/app/*` and `/admin/*` are expected to redirect to `/login`.

## Real Binance Test Checklist

Before your first live Binance run:

1. Use a low-balance Binance account or dedicated sub-account for the first pass.
2. Start the platform with Docker Compose first, then register the user account that will trade.
3. Make sure that user has an active membership before strategy start. The fastest internal test path is to open `/admin/memberships` with a `super_admin` account and use `Open membership` or `Extend membership`.
4. If you want to test real billing instead of admin activation, fill `/admin/address-pools` first so each chain has enabled deposit addresses.
5. In Binance, create an API key with read + trading permissions only. Keep withdrawal permission disabled.
6. If you enable Binance IP whitelisting, whitelist the public egress IP of this server before saving the key in `/app/exchange`.
7. If you plan to test futures, enable the relevant futures permission in Binance and turn on Hedge Mode in the Binance account before running strategy pre-flight.
8. Save the API key in `/app/exchange`, run the built-in connection test, then create a very small draft strategy such as Spot `BTCUSDT` before trying larger budgets.

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

The smoke script provides route reachability and smoke coverage through Nginx, including:

- `http://localhost:8080/`
- `http://localhost:8080/api/healthz`
- `http://localhost:8080/help/getting-started`
- `http://localhost:8080/app/dashboard`
- `http://localhost:8080/admin/dashboard`

The `/app/*` and `/admin/*` checks confirm those routes stay reachable in the release path. Anonymous requests may still redirect to `/login`; this smoke coverage does not prove full auth-gate semantics.
