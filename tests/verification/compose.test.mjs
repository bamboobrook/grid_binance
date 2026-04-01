import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("compose and docs assets exist", () => {
  for (const path of [
    "deploy/docker/docker-compose.yml",
    "deploy/docker/api-server.Dockerfile",
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
  assert.match(compose, /web:/);
  assert.match(compose, /nginx:/);
  assert.match(compose, /prometheus:/);
  assert.match(compose, /api-server\.Dockerfile/);
  assert.match(compose, /web\.Dockerfile/);
});

test("smoke script validates nginx and api entrypoints", () => {
  const script = fs.readFileSync("scripts/smoke.sh", "utf8");

  assert.match(script, /^\s*compose up -d --build\s*$/m);
  assert.match(script, /wait_for_url "http:\/\/localhost:8080\/" "nginx web entrypoint"/);
  assert.match(script, /wait_for_url "http:\/\/localhost:8080\/api\/healthz" "api health entrypoint"/);
});

test("api server container startup fails before placeholder health service when binary fails", () => {
  const dockerfile = fs.readFileSync("deploy/docker/api-server.Dockerfile", "utf8");

  assert.match(dockerfile, /\/usr\/local\/bin\/api-server\s+&&\s+exec python -m http\.server 8080/);
  assert.doesNotMatch(dockerfile, /\/usr\/local\/bin\/api-server\s*;\s*exec python -m http\.server 8080/);
});

test("docker ignore trims release build artifacts from docker context", () => {
  const dockerignore = fs.readFileSync(".dockerignore", "utf8");

  assert.match(dockerignore, /^\.git$/m);
  assert.match(dockerignore, /^\.next$/m);
  assert.match(dockerignore, /^node_modules$/m);
  assert.match(dockerignore, /^target$/m);
  assert.match(dockerignore, /^apps\/web\/test-results$/m);
});
