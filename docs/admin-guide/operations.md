# Admin Guide: Operations

## Scope

This release hardening task covers deployment assets, routing and operational checks, and the admin control surfaces shipped in V1. The admin app exposes both shared pages and `super_admin`-only control areas, so day-2 checks must confirm routing and RBAC behavior together.

## Admin Entry Points

- Public admin traffic is served through `http://localhost:8080`.
- Monitoring is served through `http://localhost:9090`.
- The API health endpoint is exposed through Nginx at `/api/healthz`.

## Day-2 Checks

1. Confirm containers are running with `docker compose --env-file .env -f deploy/docker/docker-compose.yml ps`.
2. Confirm Nginx routing with `curl -fsS http://localhost:8080/`.
3. Confirm API routing with `curl -fsS http://localhost:8080/api/healthz`.
4. Confirm Prometheus is reachable with `curl -fsS http://localhost:9090/-/ready`.
5. Confirm the admin nav exposes the full page map after login: Dashboard, Users, Memberships, Deposits, Address pools, Templates, Strategies, Sweeps, Audit, and System.
6. For a `super_admin` session, confirm `/admin/memberships`, `/admin/address-pools`, `/admin/templates`, `/admin/sweeps`, `/admin/audit`, and `/admin/system` all load with their expected control surfaces.
7. For an `operator_admin` session, confirm `/admin/deposits`, `/admin/users`, `/admin/strategies`, and `/admin/dashboard` remain reachable, `/admin/system` is read-only, and `/admin/audit` redirects away.

## Admin RBAC Reference

- Shared admin pages for both roles: `/admin/dashboard`, `/admin/users`, `/admin/deposits`, `/admin/strategies`.
- `operator_admin` read-only page: `/admin/system`.
- `super_admin` control surfaces: `/admin/memberships`, `/admin/address-pools`, `/admin/templates`, `/admin/sweeps`, `/admin/audit`.
- Do not treat `operator_admin` as authorized to manage pricing, membership lifecycle mutations, address inventory, templates, sweeps, or audit review.

## Log Collection

- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f nginx`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f web`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f api-server`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f prometheus`

## Config Reloads

- After editing Nginx config, run `docker compose --env-file .env -f deploy/docker/docker-compose.yml restart nginx`.
- After editing Prometheus rules, run `curl -X POST http://localhost:9090/-/reload`.
