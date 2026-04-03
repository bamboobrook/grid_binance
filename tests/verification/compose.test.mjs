import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function readComposeServiceBlock(compose, service) {
  const escapedService = service.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`^  ${escapedService}:\\n([\\s\\S]*?)(?=^  [a-z0-9-]+:|^volumes:)`, "m");
  const match = compose.match(pattern);
  assert.ok(match, `compose should contain a scoped block for ${service}`);
  return match[0];
}

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

test("release compose wires postgres redis persistence and required auth env for api-server and web", () => {
  const compose = fs.readFileSync("deploy/docker/docker-compose.yml", "utf8");
  const apiServerBlock = readComposeServiceBlock(compose, "api-server");
  const webBlock = readComposeServiceBlock(compose, "web");

  assert.match(compose, /^  postgres:$/m);
  assert.match(compose, /^  redis:$/m);
  assert.match(apiServerBlock, /\s+environment:\n(?:.*\n)*?\s+DATABASE_URL:\s+\$\{DATABASE_URL:\?[^}]+\}/);
  assert.match(apiServerBlock, /\s+environment:\n(?:.*\n)*?\s+REDIS_URL:\s+\$\{REDIS_URL:\?[^}]+\}/);
  assert.match(apiServerBlock, /\s+environment:\n(?:.*\n)*?\s+SESSION_TOKEN_SECRET:\s+\$\{SESSION_TOKEN_SECRET:\?[^}]+\}/);
  assert.match(apiServerBlock, /\s+environment:\n(?:.*\n)*?\s+ADMIN_EMAILS:\s+\$\{ADMIN_EMAILS:\?[^}]+\}/);
  assert.match(webBlock, /\s+environment:\n(?:.*\n)*?\s+SESSION_TOKEN_SECRET:\s+\$\{SESSION_TOKEN_SECRET:\?[^}]+\}/);
  assert.match(webBlock, /\s+environment:\n(?:.*\n)*?\s+AUTH_API_BASE_URL:\s+http:\/\/api-server:8080/);
  assert.match(compose, /^volumes:\n(?:.*\n)*?\s+postgres-data:\s*$/m);
  assert.match(compose, /^volumes:\n(?:.*\n)*?\s+redis-data:\s*$/m);
});

test("env example documents release-critical postgres redis and auth settings", () => {
  const envExample = fs.readFileSync(".env.example", "utf8");

  assert.match(envExample, /^DATABASE_URL=postgres:\/\/postgres:postgres@postgres:5432\/grid_binance$/m);
  assert.match(envExample, /^REDIS_URL=redis:\/\/redis:6379\/0$/m);
  assert.match(envExample, /^DATABASE_URL=/m);
  assert.match(envExample, /^REDIS_URL=/m);
  assert.match(envExample, /^SESSION_TOKEN_SECRET=/m);
  assert.match(envExample, /^ADMIN_EMAILS=/m);
  assert.match(envExample, /^INTERNAL_SHARED_SECRET=/m);
  assert.doesNotMatch(envExample, /^APP_DB_PATH=/m);
  assert.doesNotMatch(envExample, /^BINANCE_API_KEY=/m);
  assert.doesNotMatch(envExample, /^BINANCE_API_SECRET=/m);
});

test("smoke script validates nginx and api entrypoints and supports explicit env-file override", () => {
  const script = fs.readFileSync("scripts/smoke.sh", "utf8");

  assert.match(script, /^\s*compose up -d --build\s*$/m);
  assert.match(script, /GRID_BINANCE_ENV_FILE/);
  assert.match(script, /docker compose --env-file "\$ENV_FILE" -f "\$COMPOSE_DIR\/docker-compose\.yml"/);
  assert.match(script, /wait_for_url "http:\/\/localhost:8080\/" "nginx web entrypoint"/);
  assert.match(script, /wait_for_url "http:\/\/localhost:8080\/api\/healthz" "api health entrypoint"/);
});

test("smoke script keeps .env as the default release env source and explicitly verifies postgres redis and billing listener health", () => {
  const script = fs.readFileSync("scripts/smoke.sh", "utf8");

  assert.match(script, /INTERNAL_SHARED_SECRET/);
  assert.match(script, /DEFAULT_ENV_FILE="\$ROOT_DIR\/\.env"/);
  assert.doesNotMatch(script, /FALLBACK_ENV_FILE/);
  assert.match(script, /compose ps -q "\$service"/);
  assert.match(script, /wait_for_service_health postgres/);
  assert.match(script, /wait_for_service_health redis/);
  assert.match(script, /wait_for_service_health billing-chain-listener/);
  assert.match(
    script,
    /docker inspect -f '\{\{if \.State\.Health\}\}\{\{\.State\.Health\.Status\}\}\{\{else\}\}\{\{\.State\.Status\}\}\{\{end\}\}' "\$billing_listener_container"/,
  );
});

test("nginx config and smoke workflow refresh upstream resolution after web container recreation", () => {
  const script = fs.readFileSync("scripts/smoke.sh", "utf8");
  const nginxConfig = fs.readFileSync("deploy/nginx/default.conf", "utf8");

  assert.match(script, /compose restart nginx/);
  assert.match(nginxConfig, /resolver 127\.0\.0\.11/);
  assert.match(nginxConfig, /set \$api_upstream api-server:8080;/);
  assert.match(nginxConfig, /set \$web_upstream web:3000;/);
  assert.match(nginxConfig, /rewrite \^\/api\/(.*)\$ \/\$1 break;/);
  assert.match(nginxConfig, /proxy_pass http:\/\/\$api_upstream;/);
  assert.match(nginxConfig, /proxy_pass http:\/\/\$web_upstream;/);
});

test("compose docs consistently require explicit root env file and quoted web healthcheck", () => {
  const compose = fs.readFileSync("deploy/docker/docker-compose.yml", "utf8");
  const deploymentGuide = fs.readFileSync("docs/deployment/docker-compose.md", "utf8");
  const userGuide = fs.readFileSync("docs/user-guide/getting-started.md", "utf8");
  const adminGuide = fs.readFileSync("docs/admin-guide/operations.md", "utf8");

  assert.match(
    compose,
    /-\s+'fetch\("http:\/\/127\.0\.0\.1:3000"\)\.then\(\(res\) => process\.exit\(res\.ok \? 0 : 1\)\)\.catch\(\(\) => process\.exit\(1\)\)'/,
  );
  assert.match(deploymentGuide, /docker compose --env-file \.env -f deploy\/docker\/docker-compose\.yml up -d --build/);
  assert.match(deploymentGuide, /docker compose --env-file \.env -f deploy\/docker\/docker-compose\.yml down -v/);
  assert.match(deploymentGuide, /postgres-data/);
  assert.match(deploymentGuide, /redis-data/);
  assert.match(deploymentGuide, /prometheus-data/);
  assert.match(deploymentGuide, /127\.0\.0\.1:5432/);
  assert.match(deploymentGuide, /127\.0\.0\.1:6379/);
  assert.match(deploymentGuide, /INTERNAL_SHARED_SECRET/);
  assert.match(userGuide, /docker compose --env-file \.env -f deploy\/docker\/docker-compose\.yml up -d --build/);
  assert.match(userGuide, /cargo run -p api-server/);
  assert.doesNotMatch(userGuide, /sqlite/i);
  assert.match(adminGuide, /docker compose --env-file \.env -f deploy\/docker\/docker-compose\.yml ps/);
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
  assert.match(apiMain, /configured_database_url\(\)/);
  assert.match(apiMain, /configured_redis_url\(\)/);
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
