# User Guide: Getting Started

## Purpose

This release packages the public web shell and API behind Nginx and exposes the app at `http://localhost:8080`.

## First Run

1. Copy `.env.example` to `.env`.
2. Set at least `APP_DB_PATH`, `SESSION_TOKEN_SECRET`, and `ADMIN_EMAILS` in `.env`.
3. Start the stack with `docker compose -f deploy/docker/docker-compose.yml up -d --build`.
4. Open `http://localhost:8080`.
5. Use the public entry points:
   - `/register` for registration entry
   - `/login` for login
   - `/help/expiry-reminder` for the help center example article

## What Is Included

- The user-facing Next.js app
- Nginx reverse proxy in front of the app
- The API health endpoint at `/api/healthz`
- SQLite persistence in the Docker volume `api-server-data`, with the default file path `/var/lib/grid-binance/app.db`

## Smoke Check

Run `./scripts/smoke.sh` after the stack is up. It verifies the main web entry point and the API health endpoint routed through Nginx.
