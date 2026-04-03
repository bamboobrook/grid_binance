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
5. Confirm the admin nav matches the signed-in role after login.
6. For a `super_admin` session, confirm the nav exposes Dashboard, Users, Memberships, Deposits, Address pools, Templates, Strategies, Sweeps, Audit, and System, and that `/admin/memberships`, `/admin/address-pools`, `/admin/templates`, `/admin/sweeps`, `/admin/audit`, and `/admin/system` all load with their expected control surfaces.
7. For an `operator_admin` session, confirm the nav exposes Dashboard, Users, Memberships, Deposits, Address pools, Strategies, Sweeps, and System, confirm `/admin/memberships`, `/admin/address-pools`, `/admin/sweeps`, `/admin/deposits`, `/admin/users`, `/admin/strategies`, `/admin/system`, and `/admin/dashboard` remain reachable, confirm write actions inside Memberships, Address pools, Sweeps, and System remain restricted to `super_admin`, and confirm `/admin/templates` and `/admin/audit` stay hidden from nav and reject direct access.

## Admin RBAC Reference

- Shared admin navigation for both roles: `/admin/dashboard`, `/admin/users`, `/admin/memberships`, `/admin/deposits`, `/admin/address-pools`, `/admin/strategies`, `/admin/sweeps`, `/admin/system`.
- `/admin/templates` remains a `super_admin`-only route and stays hidden from `operator_admin` navigation.
- `operator_admin` can review Memberships, Address pools, Sweeps, and System, but the current product keeps write actions on those pages restricted to `super_admin`.
- `operator_admin` should still be treated as operationally authorized for deposit handling on `/admin/deposits`.
- `super_admin` gets the full control surface for pricing and plans, membership lifecycle actions, address inventory changes, template changes, sweep requests, system confirmation policy changes, and audit review.
- `/admin/audit` remains a `super_admin`-only route and must stay hidden/restricted for `operator_admin`.
- `/admin/templates` is also `super_admin`-only and should reject direct `operator_admin` access.

## Log Collection

- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f nginx`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f web`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f api-server`
- `docker compose --env-file .env -f deploy/docker/docker-compose.yml logs -f prometheus`

## Config Reloads

- After editing Nginx config, run `docker compose --env-file .env -f deploy/docker/docker-compose.yml restart nginx`.
- After editing Prometheus rules, run `curl -X POST http://localhost:9090/-/reload`.
