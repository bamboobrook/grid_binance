# Deployment Guide: Docker Compose

## Release Model

V1 ships as a Docker Compose deployment fronted by Nginx.

Public runtime entrypoints:

- `http://localhost:8080/` for the public site
- `http://localhost:8080/login` for auth entry
- `http://localhost:8080/app/*` for the user app
- `http://localhost:8080/admin/*` for the admin app
- `http://localhost:8080/help/*` for repository-backed help articles
- `http://localhost:8080/api/healthz` for API health through Nginx
- `http://localhost:9090` for Prometheus

## Prerequisites

- Docker Engine with Compose support
- enough local CPU, memory, and disk to build Rust and Next.js images
- a repository-root `.env` file

Start from `.env.example` and then review `docs/deployment/env-and-secrets.md` before startup.

## Start

From the repository root:

```bash
cp .env.example .env
docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build
```

This release depends on PostgreSQL and Redis. `DATABASE_URL`, `REDIS_URL`, `SESSION_TOKEN_SECRET`, `EXCHANGE_CREDENTIALS_MASTER_KEY`, `ADMIN_EMAILS`, and `TELEGRAM_BOT_BIND_SECRET` must be present before the stack starts.

`.env.example` is compose-oriented. Inside the compose network, `postgres` and `redis` resolve as service names. If you run services outside compose, override them to host-reachable values such as `postgres://postgres:postgres@127.0.0.1:5432/grid_binance` and `redis://127.0.0.1:6379/0`.

## Included Services

- `postgres` for relational runtime data
- `redis` for runtime coordination and cache usage
- `api-server` for auth, billing, admin, reporting, and integration APIs
- `web` for the Next.js public, user, admin, and help-center UI
- `nginx` for the commercial runtime entrypoint on `localhost:8080`
- `prometheus` for baseline monitoring on `localhost:9090`

## Verification

Run both checks after startup:

```bash
node --test tests/verification/*.test.mjs
./scripts/smoke.sh
```

`./scripts/smoke.sh` should validate the commercial runtime path through Nginx, not only the root page. The expected checks include:

- root web entrypoint
- API health entrypoint
- repository-backed help entrypoint
- user app route gate at `/app/dashboard`
- admin app route gate at `/admin/dashboard`

## Stop

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml down
```

Use `docker compose --env-file .env -f deploy/docker/docker-compose.yml down -v` only when you intentionally want to remove persisted PostgreSQL and Redis volumes.
