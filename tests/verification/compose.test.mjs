import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("compose and docs assets exist", () => {
  for (const path of [
    "deploy/docker/docker-compose.yml",
    "deploy/docker/api-server.Dockerfile",
    "deploy/docker/rust-service.Dockerfile",
    "deploy/docker/web.Dockerfile",
    "deploy/nginx/default.conf",
    "deploy/monitoring/prometheus.yml",
    "scripts/smoke.sh",
    "docs/user-guide/getting-started.md",
    "docs/admin-guide/operations.md",
    "docs/deployment/docker-compose.md",
  ]) {
    assert.ok(fs.existsSync(path), `${path} should exist`);
  }
});

test("compose references the expected release services", () => {
  const compose = fs.readFileSync("deploy/docker/docker-compose.yml", "utf8");

  assert.match(compose, /api-server:/);
  assert.match(compose, /trading-engine:/);
  assert.match(compose, /scheduler:/);
  assert.match(compose, /market-data-gateway:/);
  assert.match(compose, /billing-chain-listener:/);
  assert.match(compose, /web:/);
  assert.match(compose, /nginx:/);
  assert.match(compose, /prometheus:/);
  assert.match(compose, /api-server\.Dockerfile/);
  assert.match(compose, /web\.Dockerfile/);
  assert.match(compose, /rust-service\.Dockerfile/);
});

test("release compose wires sqlite persistence and required auth env for api-server and web", () => {
  const compose = fs.readFileSync("deploy/docker/docker-compose.yml", "utf8");

  assert.match(compose, /api-server:\n(?:.*\n)*?\s+environment:\n(?:.*\n)*?\s+APP_DB_PATH:\s+\$\{APP_DB_PATH(?::\?[^}]+)?\}/);
  assert.match(compose, /api-server:\n(?:.*\n)*?\s+environment:\n(?:.*\n)*?\s+SESSION_TOKEN_SECRET:\s+\$\{SESSION_TOKEN_SECRET:\?[^}]+\}/);
  assert.match(compose, /api-server:\n(?:.*\n)*?\s+environment:\n(?:.*\n)*?\s+ADMIN_EMAILS:\s+\$\{ADMIN_EMAILS:\?[^}]+\}/);
  assert.match(compose, /web:\n(?:.*\n)*?\s+environment:\n(?:.*\n)*?\s+SESSION_TOKEN_SECRET:\s+\$\{SESSION_TOKEN_SECRET:\?[^}]+\}/);
  assert.match(compose, /web:\n(?:.*\n)*?\s+environment:\n(?:.*\n)*?\s+AUTH_API_BASE_URL:\s+http:\/\/api-server:8080/);
  assert.match(compose, /api-server:\n(?:.*\n)*?\s+volumes:\n(?:.*\n)*?\s+-\s+api-server-data:\/var\/lib\/grid-binance/);
  assert.match(compose, /^volumes:\n(?:.*\n)*?\s+api-server-data:\s*$/m);
});

test("env example documents release-critical sqlite and auth settings", () => {
  const envExample = fs.readFileSync(".env.example", "utf8");

  assert.match(envExample, /^APP_DB_PATH=/m);
  assert.match(envExample, /^SESSION_TOKEN_SECRET=/m);
  assert.match(envExample, /^ADMIN_EMAILS=/m);
});

test("smoke script validates nginx and api entrypoints", () => {
  const script = fs.readFileSync("scripts/smoke.sh", "utf8");

  assert.match(script, /^\s*compose up -d --build\s*$/m);
  assert.match(script, /wait_for_url "http:\/\/localhost:8080\/" "nginx web entrypoint"/);
  assert.match(script, /wait_for_url "http:\/\/localhost:8080\/api\/healthz" "api health entrypoint"/);
});

test("api server container runs the real Rust process without placeholder http server", () => {
  const dockerfile = fs.readFileSync("deploy/docker/api-server.Dockerfile", "utf8");

  assert.match(dockerfile, /CMD \["\/usr\/local\/bin\/api-server"\]/);
  assert.doesNotMatch(dockerfile, /python -m http\.server/);
  assert.doesNotMatch(dockerfile, /grid-binance api placeholder/);
});

test("rust service entrypoints are long-running health probe servers instead of bootstrap printlns", () => {
  const apiMain = fs.readFileSync("apps/api-server/src/main.rs", "utf8");
  assert.match(apiMain, /api_server::app_with_persistent_state\(/);
  assert.match(apiMain, /configured_db_path\(/);
  assert.match(apiMain, /\/healthz/);
  assert.doesNotMatch(apiMain, /println!\("bootstrap"\)/);

  for (const path of [
    "apps/trading-engine/src/main.rs",
    "apps/scheduler/src/main.rs",
    "apps/market-data-gateway/src/main.rs",
    "apps/billing-chain-listener/src/main.rs",
  ]) {
    const source = fs.readFileSync(path, "utf8");
    assert.match(source, /\/healthz/);
    assert.doesNotMatch(source, /println!\("bootstrap"\)/);
  }
});

test("prometheus scrapes every Rust service health endpoint", () => {
  const prometheus = fs.readFileSync("deploy/monitoring/prometheus.yml", "utf8");

  for (const target of [
    "api-server:8080",
    "trading-engine:8081",
    "scheduler:8082",
    "market-data-gateway:8083",
    "billing-chain-listener:8084",
  ]) {
    assert.match(prometheus, new RegExp(target.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }
  assert.match(prometheus, /metrics_path:\s*\/healthz/);
});

test("docker ignore trims release build artifacts from docker context", () => {
  const dockerignore = fs.readFileSync(".dockerignore", "utf8");

  assert.match(dockerignore, /^\.git$/m);
  assert.match(dockerignore, /^\.next$/m);
  assert.match(dockerignore, /^node_modules$/m);
  assert.match(dockerignore, /^target$/m);
  assert.match(dockerignore, /^apps\/web\/test-results$/m);
});
