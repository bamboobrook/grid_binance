# Getting Started

Use the in-app help route `/app/help?article=getting-started` for the authenticated workspace version of this guide, or open the public help route `/help/getting-started` before login. The in-app help center also supports direct article links such as `/app/help?article=<slug>`, and the public help route supports `/help/<slug>`.

## Main Routes

- `/app/dashboard` for overview and expiry reminders
- `/app/exchange` for Binance API setup and connection tests
- `/app/strategies` for list, batch actions, and stop-all
- `/app/strategies/new` for draft creation
- `/app/orders` for fills, order history, and account activity review
- `/app/billing` for membership orders and payment instructions
- `/app/telegram` for Telegram bot binding and notification delivery status
- `/app/security` for password and TOTP actions

## First Run Path

1. Register and verify your email.
2. Log in and open the security center.
3. If you are a configured admin account, open `/admin-bootstrap` first and finish the initial TOTP bootstrap before admin login.
4. Open `/app/exchange` and save your Binance API key and secret.
5. Run the connection test.
6. Open `/app/billing` and create a membership order.
7. Send the exact stablecoin amount to the assigned address.
8. After membership is active, open `/app/strategies/new` and create the draft.
9. Run pre-flight, then start the strategy.

## Local Stack

- Start the commercial stack with `docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build`
- Stop and remove it with `docker compose --env-file .env -f deploy/docker/docker-compose.yml down -v`
- Local Rust-only bring-up still uses `cargo run -p api-server`

## Binance API Checklist

- Enable read and trading permissions only.
- Keep withdrawal permission disabled.
- Futures strategies require hedge mode.
- One user can bind only one Binance account.

## Strategy Checklist

- Use fuzzy symbol search.
- Choose amount mode: quote amount or base asset quantity.
- Use batch ladder builder for fast spacing and take-profit setup.
- Switch to custom JSON for every-grid overrides.
- Pause before editing any running strategy.
