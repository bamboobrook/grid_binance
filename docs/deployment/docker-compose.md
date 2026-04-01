# Deployment Guide: Docker Compose

## Prerequisites

- Docker Engine with Compose support
- Enough local resources to build the Rust and Next.js images
- A `.env` file at the repository root. Start from `.env.example` and set these release-critical values:
  - `APP_DB_PATH=/var/lib/grid-binance/app.db`
  - `SESSION_TOKEN_SECRET=<long-random-secret>`
  - `ADMIN_EMAILS=admin@example.com`

## Start

From the repository root:

```bash
cp .env.example .env
```

Edit `.env` before startup. `SESSION_TOKEN_SECRET` and `ADMIN_EMAILS` are required by compose and the stack will fail fast if either is missing. Keep `APP_DB_PATH` under `/var/lib/grid-binance/` unless you also change the mounted volume path.

Run the stack from the repository root:

```bash
docker compose -f deploy/docker/docker-compose.yml up -d --build
```

## Included Services

- `api-server`: builds the Rust `api-server` binary, serves the application API on port 8080 inside the network, and stores SQLite data at `APP_DB_PATH`
- `web`: builds the Next.js application and serves it on port 3000 inside the network
- `nginx`: reverse proxy exposed on `localhost:8080`
- `prometheus`: baseline monitoring exposed on `localhost:9090`

## Persistence

- Compose mounts the named volume `api-server-data` at `/var/lib/grid-binance` inside `api-server`
- With the default example env, SQLite lives at `/var/lib/grid-binance/app.db`
- `docker compose down` keeps the SQLite volume; use `docker compose -f deploy/docker/docker-compose.yml down -v` only when you intentionally want to remove persisted data

## Verification

```bash
node --test tests/verification/*.test.mjs
./scripts/smoke.sh
```

## Stop

```bash
docker compose -f deploy/docker/docker-compose.yml down
```
