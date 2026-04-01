import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("runtime storage shape requires postgres and redis services in compose", () => {
  const compose = fs.readFileSync("deploy/docker/docker-compose.yml", "utf8");

  assert.match(compose, /^  postgres:$/m);
  assert.match(compose, /^  redis:$/m);
  assert.match(
    compose,
    /api-server:\n(?:.*\n)*?\s+environment:\n(?:.*\n)*?\s+DATABASE_URL:\s+\$\{DATABASE_URL:\?[^}]+\}/,
  );
  assert.match(
    compose,
    /api-server:\n(?:.*\n)*?\s+environment:\n(?:.*\n)*?\s+REDIS_URL:\s+\$\{REDIS_URL:\?[^}]+\}/,
  );
});

test("deployment docs no longer describe sqlite as runtime storage", () => {
  const guide = fs.readFileSync("docs/deployment/docker-compose.md", "utf8");

  assert.doesNotMatch(guide, /sqlite/i);
  assert.match(guide, /PostgreSQL/);
  assert.match(guide, /Redis/);
});

test("shared-db no longer exposes sqlite runtime helpers", () => {
  const sharedDb = fs.readFileSync("crates/shared-db/src/lib.rs", "utf8");

  assert.doesNotMatch(sharedDb, /\brusqlite\b/);
  assert.doesNotMatch(sharedDb, /\bopen_in_memory\b/);
  assert.doesNotMatch(sharedDb, /\bin_memory\s*\(/);
  assert.doesNotMatch(sharedDb, /bootstrap_label\(\)\s*->\s*&'static str\s*\{\s*"sqlite"/);
});
