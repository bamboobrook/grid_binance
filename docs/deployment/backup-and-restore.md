# Deployment Guide: Backup And Restore

## Scope

V1 runs with PostgreSQL and Redis as stateful runtime dependencies. Backup and restore procedures must preserve the commercial runtime path, billing state, auth data, and operational settings.

## What To Protect

- PostgreSQL application data in the `postgres-data` volume
- Redis append-only data in the `redis-data` volume
- repository-root `.env` values required to decrypt and validate runtime state

## Backup Guidance

1. Record the deployed commit and compose file path: `deploy/docker/docker-compose.yml`.
2. Confirm the stack is healthy with `docker compose --env-file .env -f deploy/docker/docker-compose.yml ps`.
3. Take a PostgreSQL backup using a consistent database dump process.
4. Preserve Redis persistence data when runtime coordination state matters to incident recovery.
5. Store `.env` securely alongside backup metadata, because restore without the same secrets can invalidate sessions and encrypted credentials.

## Restore Guidance

1. Restore `.env` first.
2. Recreate the stack with `docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build`.
3. Restore PostgreSQL data.
4. Restore Redis persistence only when that runtime state is intentionally part of recovery.
5. Re-run `./scripts/smoke.sh` after restore.

## Post-Restore Checks

After restore, verify:

- `http://localhost:8080/`
- `http://localhost:8080/api/healthz`
- `http://localhost:8080/help/getting-started`
- `/app/*` auth gate behavior
- `/admin/*` auth gate behavior
- Prometheus reachability on `http://localhost:9090`
