#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_DIR="$ROOT_DIR/deploy/docker"
DEFAULT_ENV_FILE="$ROOT_DIR/.env"
ENV_FILE="${GRID_BINANCE_ENV_FILE:-$DEFAULT_ENV_FILE}"
# Commercial runtime path: deploy/docker/docker-compose.yml

if [[ ! -f "$ENV_FILE" ]]; then
  echo "smoke check failed: env file not found: $ENV_FILE" >&2
  exit 1
fi

if ! grep -Eq "^INTERNAL_SHARED_SECRET=" "$ENV_FILE"; then
  echo "smoke check failed: env file $ENV_FILE is missing INTERNAL_SHARED_SECRET" >&2
  exit 1
fi

compose() {
  if docker compose version >/dev/null 2>&1; then
    docker compose --env-file "$ENV_FILE" -f "$COMPOSE_DIR/docker-compose.yml" "$@"
    return 0
  fi

  if command -v docker-compose >/dev/null 2>&1; then
    docker-compose --env-file "$ENV_FILE" -f "$COMPOSE_DIR/docker-compose.yml" "$@"
    return 0
  fi

  echo "docker compose is unavailable; install the Compose plugin or docker-compose" >&2
  return 1
}

wait_for_url() {
  local url="$1"
  local label="$2"

  for _ in $(seq 1 30); do
    if curl -fsS "$url" >/dev/null; then
      return 0
    fi
    sleep 2
  done

  echo "smoke check failed: $label ($url)" >&2
  return 1
}

wait_for_service_health() {
  local service="$1"
  local billing_listener_container=""
  local billing_listener_status=""

  for _ in $(seq 1 30); do
    billing_listener_container="$(compose ps -q "$service" | tr -d "\r")"
    if [[ -n "$billing_listener_container" ]]; then
      billing_listener_status="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$billing_listener_container" 2>/dev/null || true)"
      if [[ "$billing_listener_status" == "healthy" || "$billing_listener_status" == "running" ]]; then
        return 0
      fi
    fi
    sleep 2
  done

  compose ps "$service" || true
  echo "smoke check failed: $service is not healthy" >&2
  return 1
}

# Preferred path from the release checklist: docker compose up -d --build
compose up -d --build
compose restart nginx

wait_for_url "http://localhost:8080/" "nginx web entrypoint"
wait_for_url "http://localhost:8080/api/healthz" "api health entrypoint"
wait_for_url "http://localhost:8080/help/getting-started" "repository help entrypoint"
wait_for_url "http://localhost:8080/app/dashboard" "user commercial runtime path"
wait_for_url "http://localhost:8080/admin/dashboard" "admin commercial runtime path"

compose ps
wait_for_service_health postgres
wait_for_service_health redis
wait_for_service_health billing-chain-listener
