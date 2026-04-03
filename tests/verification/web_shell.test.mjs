import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("web shell and workspace hygiene are present", () => {
  for (const path of [
    "apps/web/package.json",
    "apps/web/eslint.config.mjs",
    "apps/web/postcss.config.cjs",
    "apps/web/src/app/layout.tsx",
    "apps/web/src/app/(public)/layout.tsx",
    "apps/web/src/app/(public)/page.tsx",
    "Cargo.lock",
    "pnpm-lock.yaml",
  ]) {
    assert.ok(fs.existsSync(path), `${path} should exist`);
  }

  const gitignore = fs.readFileSync(".gitignore", "utf8");
  assert.match(gitignore, /^target\/$/m);

  const rootPackage = JSON.parse(fs.readFileSync("package.json", "utf8"));
  assert.ok(rootPackage.scripts["build:web"]);
  assert.ok(rootPackage.scripts.build);

  const webPackage = JSON.parse(fs.readFileSync("apps/web/package.json", "utf8"));
  assert.equal(webPackage.scripts.lint, "eslint .");
  assert.equal(webPackage.type, "module");
  assert.ok(!fs.existsSync("apps/web/postcss.config.js"));

  const pnpmLock = fs.readFileSync("pnpm-lock.yaml", "utf8");
  assert.match(pnpmLock, /^  apps\/web:$/m);
});
