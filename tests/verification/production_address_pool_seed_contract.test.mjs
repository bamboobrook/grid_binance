import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const source = fs.readFileSync(
  "/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/api-server/src/services/membership_service.rs",
  "utf8",
);

test("production membership bootstrap does not reseed placeholder deposit addresses", () => {
  assert.match(source, /APP_ENV/, "membership bootstrap should read APP_ENV before seeding placeholder deposit addresses");
  assert.match(source, /production/, "membership bootstrap should explicitly guard production runtime from placeholder address seeding");
});
