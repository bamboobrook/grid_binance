# Admin Guide: Operations

## Scope

This release hardening task covers deployment assets, routing and operational checks, and the new admin commercial business flows shipped in V1. Admin operators can now manage membership pricing, membership lifecycle actions, abnormal deposit handling, address pool inventory, treasury sweep requests, strategy/template oversight, system confirmation policy, and audit review from the admin surface.

## Admin Entry Points

- Public admin traffic is served through `http://localhost:8080`.
- Monitoring is served through `http://localhost:9090`.
- The API health endpoint is exposed through Nginx at `/api/healthz`.

## Day-2 Checks

1. Confirm containers are running with `docker compose --env-file .env -f deploy/docker/docker-compose.yml ps`.
2. Confirm Nginx routing with `curl -fsS http://localhost:8080/`.
3. Confirm API routing with `curl -fsS http://localhost:8080/api/healthz`.
4. Confirm Prometheus is reachable with `curl -fsS http://localhost:9090/-/ready`.
5. Confirm admin commercial flows are reachable after login by checking Memberships, Deposits, Address pools, Templates, Strategies, System, and Audit in the admin nav.

## Log Collection

- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f nginx`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f web`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f api-server`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f prometheus`

## Config Reloads

- After editing Nginx config, run `docker compose --env-file .env -f deploy/docker/docker-compose.yml restart nginx`.
- After editing Prometheus rules, run `curl -X POST http://localhost:9090/-/reload`.
