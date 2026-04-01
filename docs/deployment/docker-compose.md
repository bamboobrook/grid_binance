# Deployment Guide: Docker Compose

## Prerequisites

- Docker Engine with Compose support
- Enough local resources to build the Rust and Next.js images

## Start

Run the stack from the repository root:

```bash
docker compose -f deploy/docker/docker-compose.yml up -d --build
```

## Included Services

- `api-server`: builds the Rust `api-server` binary and serves a deployment health placeholder on port 8080 inside the network
- `web`: builds the Next.js application and serves it on port 3000 inside the network
- `nginx`: reverse proxy exposed on `localhost:8080`
- `prometheus`: baseline monitoring exposed on `localhost:9090`

## Verification

```bash
node --test tests/verification/*.test.mjs
./scripts/smoke.sh
```

## Stop

```bash
docker compose -f deploy/docker/docker-compose.yml down
```

## Known Minimalism

This task intentionally ships the smallest verifiable deployment baseline. The API container currently boots the compiled binary once and then serves a static health endpoint so the release pipeline can validate reverse proxy wiring without adding extra backend behavior.
