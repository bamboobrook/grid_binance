import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("web shell and workspace hygiene are present", () => {
  for (const path of [
    "apps/web/package.json",
    "apps/web/src/app/layout.tsx",
    "apps/web/src/app/page.tsx",
    "Cargo.lock",
  ]) {
    assert.ok(fs.existsSync(path), `${path} should exist`);
  }

  const gitignore = fs.readFileSync(".gitignore", "utf8");
  assert.match(gitignore, /^target\/$/m);

  const rootPackage = JSON.parse(fs.readFileSync("package.json", "utf8"));
  assert.ok(rootPackage.scripts["build:web"]);
  assert.ok(rootPackage.scripts.build);
});
