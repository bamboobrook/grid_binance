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
- a repository-root `.env` file for normal compose startup

Start from `.env.example` and then review `docs/deployment/env-and-secrets.md` before startup. `./scripts/smoke.sh` also accepts `GRID_BINANCE_ENV_FILE=/path/to/env` for explicit local acceptance runs, but the default release path remains the repository-root `.env` file.

## Start

From the repository root:

```bash
cp .env.example .env
docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build
```

This release depends on PostgreSQL and Redis. `DATABASE_URL`, `REDIS_URL`, `SESSION_TOKEN_SECRET`, `EXCHANGE_CREDENTIALS_MASTER_KEY`, `ADMIN_EMAILS`, `TELEGRAM_BOT_BIND_SECRET`, `INTERNAL_SHARED_SECRET`, `CHAIN_RPC_URL_ETH`, `CHAIN_RPC_URL_BSC`, and `CHAIN_RPC_URL_SOL` must be present before the stack starts. Set `BINANCE_LIVE_MODE=1` when you want the API, scheduler, trading engine, and market-data gateway to hit the real Binance endpoints instead of offline fallbacks. Add `TELEGRAM_BOT_TOKEN` if you want Telegram delivery in addition to in-app notifications. Set `SWEEP_EXECUTOR_URL` when you want `/admin/sweeps` jobs to be submitted automatically and return real tx hashes or Solana signatures. Add `SWEEP_EXECUTOR_AUTH_TOKEN` when that executor requires bearer auth. Tune `SNAPSHOT_SYNC_INTERVAL_SECS` if you want account/wallet snapshots captured more or less frequently, tune `REMINDER_INTERVAL_SECS` / `REMINDER_LOOKAHEAD_HOURS` if you want renewal reminders emitted on a different cadence, and override `BINANCE_*_WS_BASE_URL` only when routing market/user streams through a proxy or alternate endpoint.

`.env.example` is compose-oriented. Inside the compose network, `postgres` and `redis` resolve as service names. If you run services outside compose, override them to host-reachable values such as `postgres://postgres:postgres@127.0.0.1:5432/grid_binance` and `redis://127.0.0.1:6379/0`.

## Included Services

The compose stack includes 10 services in total, including 5 Rust services.

- `postgres` for relational runtime data
- `redis` for runtime coordination and cache usage
- `api-server` as the Rust auth, billing, admin, reporting, and integration API
- `trading-engine` as the Rust order execution worker
- `scheduler` as the Rust strategy scheduling and pre-flight worker
- `market-data-gateway` as the Rust market data ingestion service
- `billing-chain-listener` as the Rust billing deposit listener
- `web` for the Next.js public, user, admin, and help-center UI
- `nginx` for the commercial runtime entrypoint on `localhost:8080`
- `prometheus` for baseline monitoring on `localhost:9090`

## Verification

Run both checks after startup:

```bash
node --test tests/verification/*.test.mjs
./scripts/smoke.sh
```

If you want to smoke-test the example secrets bundle without editing your local `.env`, run `GRID_BINANCE_ENV_FILE=.env.example ./scripts/smoke.sh`.

`./scripts/smoke.sh` should provide smoke coverage for the commercial runtime path through Nginx, not only the root page. The expected checks include:

- root web entrypoint
- API health entrypoint
- repository-backed help entrypoint
- user app route reachability at `/app/dashboard`
- admin app route reachability at `/admin/dashboard`

## Stop

```bash
docker compose --env-file .env -f deploy/docker/docker-compose.yml down
```

Use `docker compose --env-file .env -f deploy/docker/docker-compose.yml down -v` only when you intentionally want to remove the named volumes `postgres-data`, `redis-data`, and `prometheus-data`.

## Martingale Backtest Worker

The compose stack may include a `backtest-worker` service for martingale Portfolio searches and two-stage backtests. Configure it through the repository-root `.env` file:

- `BACKTEST_ARTIFACT_ROOT` controls where compact JSONL artifacts and manifests are written.
- `BACKTEST_WORKER_MAX_THREADS` limits per-worker CPU usage.
- `BACKTEST_WORKER_POLL_MS` controls how often the worker polls for queued tasks and pause/cancel changes.
- `BACKTEST_MARKET_DATA_DB_PATH` points to the external market data SQLite database used by the worker. When set, the worker opens it read-only for K-line screening and aggTrades refinement; without it, martingale worker tasks fail instead of generating synthetic candidates.

Mount `BACKTEST_ARTIFACT_ROOT` on persistent storage. If API or web processes need to download or inspect artifacts, they must mount the same artifact volume or access the same host path.

When an external market data database is mounted for this worker, it is read-only input. The backtest worker should open it read-only and must not modify it with schema migrations, index creation, VACUUM, checkpoint, or cleanup jobs. If the source database is produced by another project, keep ownership of that file with the producer project and treat this stack as a read-only consumer.
