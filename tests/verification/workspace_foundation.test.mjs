import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

test("foundation workspace files exist", () => {
  for (const path of ["Cargo.toml", "package.json", "apps/api-server/src/main.rs"]) {
    assert.ok(fs.existsSync(path), `${path} should exist`);
  }
});

test("makefile test target uses a POSIX-compatible cargo env bootstrap", () => {
  const makefile = fs.readFileSync("Makefile", "utf8");
  assert.match(makefile, /^\s*\.\s+"\$\$HOME\/\.cargo\/env"\s+&&\s+cargo test --workspace$/m);
});

test("package.json test script runs the same foundation checks as make test", () => {
  const packageJson = JSON.parse(fs.readFileSync("package.json", "utf8"));
  assert.match(packageJson.scripts.test, /\.\s+"\$HOME\/\.cargo\/env"\s+&&\s+cargo test --workspace/);
  assert.match(packageJson.scripts.test, /node --test tests\/verification\/\*\.test\.mjs/);
});
