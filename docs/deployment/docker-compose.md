# Deployment Guide: Docker Compose

## Prerequisites

- Docker Engine with Compose support
- Enough local resources to build the Rust and Next.js images
- A `.env` file at the repository root. Start from `.env.example` and set these release-critical values:
  - `POSTGRES_DB=grid_binance`
  - `POSTGRES_USER=postgres`
  - `POSTGRES_PASSWORD=postgres`
  - `DATABASE_URL=postgres://postgres:postgres@postgres:5432/grid_binance`
  - `REDIS_URL=redis://redis:6379/0`
  - `SESSION_TOKEN_SECRET=<long-random-secret>`
  - `ADMIN_EMAILS=admin@example.com`

## Start

From the repository root:

```bash
cp .env.example .env
```

Edit `.env` before startup. PostgreSQL and Redis are mandatory runtime dependencies. `DATABASE_URL`, `REDIS_URL`, `SESSION_TOKEN_SECRET`, and `ADMIN_EMAILS` are required by compose and the services fail fast if any of them are missing.
`.env.example` is compose-oriented: `postgres` and `redis` resolve to service names inside the compose network. For local non-compose `cargo run`, override them to host-accessible values such as `DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/grid_binance` and `REDIS_URL=redis://127.0.0.1:6379/0`.

Run the stack from the repository root:

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build
```

## Included Services

- `postgres`: primary relational store for identity, billing, exchange, strategy, admin, and notification data
- `redis`: runtime cache and coordination store
- `api-server`: builds the Rust `api-server` binary and serves the application API on port 8080 inside the network
- `web`: builds the Next.js application and serves it on port 3000 inside the network
- `nginx`: reverse proxy exposed on `localhost:8080`
- `prometheus`: baseline monitoring exposed on `localhost:9090`

## Persistence

- Compose mounts `postgres-data` for PostgreSQL data files and `redis-data` for Redis append-only persistence
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml down` keeps both named volumes; use `docker compose --env-file .env -f deploy/docker/docker-compose.yml down -v` only when you intentionally want to remove persisted runtime state

## Verification

```bash
node --test tests/verification/*.test.mjs
./scripts/smoke.sh
```

## Stop

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml down
```
