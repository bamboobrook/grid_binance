#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_DIR="$ROOT_DIR/deploy/docker"

compose() {
  if docker compose version >/dev/null 2>&1; then
    docker compose --env-file "$ROOT_DIR/.env" -f "$COMPOSE_DIR/docker-compose.yml" "$@"
    return 0
  fi

  if command -v docker-compose >/dev/null 2>&1; then
    docker-compose --env-file "$ROOT_DIR/.env" -f "$COMPOSE_DIR/docker-compose.yml" "$@"
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

# Preferred path from the release checklist: docker compose up -d --build
compose up -d --build

wait_for_url "http://localhost:8080/" "nginx web entrypoint"
wait_for_url "http://localhost:8080/api/healthz" "api health entrypoint"

compose ps
