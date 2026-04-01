# User Guide: Getting Started

## Purpose

This release packages the public web shell behind Nginx and exposes the app at `http://localhost:8080`.

## First Run

1. Start the stack with `docker compose -f deploy/docker/docker-compose.yml up -d --build`.
2. Open `http://localhost:8080`.
3. Use the public entry points:
   - `/register` for registration entry
   - `/login` for login
   - `/help/expiry-reminder` for the help center example article

## What Is Included

- The user-facing Next.js app
- Nginx reverse proxy in front of the app
- A placeholder API health endpoint at `/api/healthz`

## Smoke Check

Run `./scripts/smoke.sh` after the stack is up. It verifies the main web entry point and the API health endpoint routed through Nginx.
