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
- `EXCHANGE_CREDENTIALS_MASTER_KEY`
- `ADMIN_EMAILS`
- `TELEGRAM_BOT_BIND_SECRET`
- `INTERNAL_SHARED_SECRET`

## Secret Ownership Rules

- `SESSION_TOKEN_SECRET` signs session tokens used by the web and API auth boundary.
- `EXCHANGE_CREDENTIALS_MASTER_KEY` is dedicated to exchange credential encryption.
- User Binance API keys are entered through `/app/exchange`; they are not supplied through the release `.env` file.
- `TELEGRAM_BOT_BIND_SECRET` is dedicated to Telegram bind and bot-facing trust checks.
- `INTERNAL_SHARED_SECRET` is dedicated to trusted internal service-to-service calls such as chain-listener ingestion.
- Do not reuse one secret for another purpose.

## Recommended Format

- Use long random values for every secret.
- Keep `ADMIN_EMAILS` as a comma-separated list of admin email addresses.
- Keep compose-local runtime URLs pointed at service names:
  - `DATABASE_URL=postgres://postgres:postgres@postgres:5432/grid_binance`
  - `REDIS_URL=redis://redis:6379/0`

## Operational Notes

- Store `.env` outside screenshots, tickets, and chat logs.
- Rotate secrets during incident response or administrator turnover.
- Treat `.env.example` as a template only; never deploy placeholder values.
- When running services outside compose, override `DATABASE_URL` and `REDIS_URL` to host-reachable addresses such as `127.0.0.1`.
