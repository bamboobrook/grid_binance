# Admin Guide: Operations

## Scope

This release hardening task focuses on deployment assets, basic routing, and operational checks. It does not add new admin business flows.

## Admin Entry Points

- Public admin traffic is served through `http://localhost:8080`.
- Monitoring is served through `http://localhost:9090`.
- The API health endpoint is exposed through Nginx at `/api/healthz`.

## Day-2 Checks

1. Confirm containers are running with `docker compose --env-file .env -f deploy/docker/docker-compose.yml ps`.
2. Confirm Nginx routing with `curl -fsS http://localhost:8080/`.
3. Confirm API routing with `curl -fsS http://localhost:8080/api/healthz`.
4. Confirm Prometheus is reachable with `curl -fsS http://localhost:9090/-/ready`.

## Log Collection

- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f nginx`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f web`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f api-server`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f prometheus`

## Config Reloads

- After editing Nginx config, run `docker compose --env-file .env -f deploy/docker/docker-compose.yml restart nginx`.
- After editing Prometheus rules, run `curl -X POST http://localhost:9090/-/reload`.
