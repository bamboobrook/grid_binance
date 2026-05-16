import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const pagePath = "/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/telegram/page.tsx";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("telegram page keeps a fresh bind code visible even when the current account is still bound", () => {
  const source = read(pagePath);
  const bindCodeBranch = source.indexOf('bindCode !== "" ? (');
  const boundBranch = source.indexOf('binding?.bound ? (');

  assert.notEqual(bindCodeBranch, -1, "telegram page should have a branch for showing a fresh bind code");
  assert.notEqual(boundBranch, -1, "telegram page should still render the bound state branch");
  assert.ok(
    bindCodeBranch < boundBranch,
    "a newly issued bind code should take precedence over the stale bound-state panel until rebinding finishes",
  );
  assert.match(
    source,
    /\/bind \{bindCode\}/,
    "telegram page should explicitly show the /bind command built from the fresh bind code",
  );
});
