# Deployment Guide: Env And Secrets

## Scope

This file defines the release-critical configuration for the Docker Compose deployment path in V1.

## Required Environment Values

At minimum, set these values in the repository-root `.env` file:

- `POSTGRES_DB`
- `POSTGRES_USER`
- `POSTGRES_PASSWORD`
- `DATABASE_URL`
- `REDIS_URL`
- `SESSION_TOKEN_SECRET`
- `AUTH_EMAIL_DELIVERY`
- `AUTH_EMAIL_FROM`
- `EXCHANGE_CREDENTIALS_MASTER_KEY`
- `ADMIN_EMAILS`
- `TELEGRAM_BOT_BIND_SECRET`
- `INTERNAL_SHARED_SECRET`
- `CHAIN_RPC_URL_ETH`
- `CHAIN_RPC_URL_BSC`
- `CHAIN_RPC_URL_SOL`

Optional but required for the selected auth email delivery path:
- `AUTH_EMAIL_SMTP_HOST`
- `AUTH_EMAIL_SMTP_PORT`
- `AUTH_EMAIL_SMTP_HELO_NAME`
- `AUTH_EMAIL_SMTP_USERNAME`
- `AUTH_EMAIL_PASSWORD`
- `AUTH_EMAIL_HTTP_URL`
- `AUTH_EMAIL_HTTP_BEARER_TOKEN`

Optional but required for real Telegram delivery or alternate Telegram API routing:
- `TELEGRAM_BOT_TOKEN`
- `TELEGRAM_API_BASE_URL`
- `BINANCE_SPOT_WS_BASE_URL`
- `BINANCE_USDM_WS_BASE_URL`
- `BINANCE_COINM_WS_BASE_URL`

Optional but required for automatic billing detection on the corresponding asset:
- `CHAIN_TOKEN_CONTRACT_ETH_USDT`
- `CHAIN_TOKEN_CONTRACT_ETH_USDC`
- `CHAIN_TOKEN_CONTRACT_BSC_USDT`
- `CHAIN_TOKEN_CONTRACT_BSC_USDC`
- `CHAIN_TOKEN_MINT_SOL_USDT`
- `CHAIN_TOKEN_MINT_SOL_USDC`
- `CHAIN_LISTENER_RPC_POLL_INTERVAL_SECS`
- `CHAIN_LISTENER_EVM_INITIAL_LOOKBACK_BLOCKS`
- `CHAIN_LISTENER_SOL_SIGNATURE_LIMIT`
- `SWEEP_EXECUTOR_URL`
- `SWEEP_EXECUTOR_AUTH_TOKEN`
- `SNAPSHOT_SYNC_INTERVAL_SECS`
- `REMINDER_INTERVAL_SECS`
- `REMINDER_LOOKAHEAD_HOURS`

## Secret Ownership Rules

- `SESSION_TOKEN_SECRET` signs session tokens used by the web and API auth boundary.
- `AUTH_EMAIL_DELIVERY` selects how registration and password reset codes are delivered. Use `smtp` for a trusted relay or `http` for a mail webhook. Persistent runtime auth rejects `capture` or missing delivery config.
- `AUTH_EMAIL_FROM` is the sender address used for verification and password reset mail.
- `AUTH_EMAIL_SMTP_HOST`, `AUTH_EMAIL_SMTP_PORT`, and `AUTH_EMAIL_SMTP_HELO_NAME` configure the SMTP relay path. `AUTH_EMAIL_SMTP_USERNAME` and `AUTH_EMAIL_PASSWORD` are optional but required when the relay enforces SMTP AUTH; when the username is omitted and a password is present, the sender address is used as the AUTH username. This implementation still expects plain SMTP without STARTTLS.
- `AUTH_EMAIL_HTTP_URL` and `AUTH_EMAIL_HTTP_BEARER_TOKEN` configure the generic JSON webhook path for external mail gateways.
- `EXCHANGE_CREDENTIALS_MASTER_KEY` is dedicated to exchange credential encryption.
- `BINANCE_LIVE_MODE` toggles real Binance REST/WS execution paths for the API, trading engine, scheduler, and market-data gateway.
- Optional Binance base URL overrides let you point those live integrations at a proxy or alternate endpoint during operations.
- User Binance API keys are entered through `/app/exchange`; they are not supplied through the release `.env` file.
- `TELEGRAM_BOT_BIND_SECRET` is dedicated to Telegram bind and bot-facing trust checks.
- `TELEGRAM_BOT_TOKEN` lets the API server, scheduler, trading engine, and billing listener send real Telegram messages to bound chats. When omitted, notifications stay in-app only.
- `TELEGRAM_API_BASE_URL` optionally points Telegram delivery at a proxy or self-hosted gateway.
- `INTERNAL_SHARED_SECRET` is dedicated to trusted internal service-to-service calls such as chain-listener ingestion.
- `CHAIN_RPC_URL_ETH`, `CHAIN_RPC_URL_BSC`, and `CHAIN_RPC_URL_SOL` point the billing listener at your production RPC providers.
- `SWEEP_EXECUTOR_URL` is the authenticated submission endpoint the billing listener uses to broadcast treasury sweep transfers and receive the real tx hash or signature.
- `SWEEP_EXECUTOR_AUTH_TOKEN` is the optional bearer token sent to the sweep executor.
- Token contract and mint values define which stablecoin transfers are considered payable on each chain.
- Do not reuse one secret for another purpose.

## Recommended Format

- Use long random values for every secret.
- Keep `ADMIN_EMAILS` as a comma-separated list of admin email addresses.
- Keep `AUTH_EMAIL_DELIVERY` aligned with the configured backend; do not leave the SMTP/HTTP fields half-filled.
- Keep compose-local runtime URLs pointed at service names:
  - `DATABASE_URL=postgres://postgres:postgres@postgres:5432/grid_binance`
  - `REDIS_URL=redis://redis:6379/0`

## Operational Notes

- Store `.env` outside screenshots, tickets, and chat logs.
- Rotate secrets during incident response or administrator turnover.
- Treat `.env.example` as a template only; never deploy placeholder values.
- When running services outside compose, override `DATABASE_URL` and `REDIS_URL` to host-reachable addresses such as `127.0.0.1`.

## Martingale Backtest Worker Configuration

Set these values when enabling the martingale backtest worker:

- `BACKTEST_ARTIFACT_ROOT`: directory where the worker writes JSONL result artifacts. In Docker Compose this should be backed by a persistent volume shared with services that need to read artifacts.
- `BACKTEST_WORKER_MAX_THREADS`: maximum CPU worker threads a single backtest worker process may use.
- `BACKTEST_WORKER_POLL_MS`: polling interval, in milliseconds, for queued task checks and pause/cancel observation.
- `BACKTEST_MARKET_DATA_DB_PATH`: path to the external SQLite market data database used for K-line screening and trade refinement. The worker opens this database read-only; if the variable is omitted, martingale worker tasks fail rather than producing synthetic candidates.

When `BACKTEST_MARKET_DATA_DB_PATH` is enabled in an environment, it must point to an external market database opened in read-only mode. Do not grant the backtest worker write privileges to that source database, and do not run migrations, index creation, VACUUM, checkpoint, or repair operations against it from this application.
