import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const source = fs.readFileSync("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/lib/auth.ts", "utf8");

test("auth web helpers derive redirect origins from forwarded host metadata", () => {
  assert.match(source, /x-forwarded-host/i, "redirect helper should read x-forwarded-host");
  assert.match(source, /x-forwarded-port/i, "redirect helper should read x-forwarded-port");
  assert.match(source, /x-forwarded-proto/i, "redirect helper should read x-forwarded-proto");
  assert.match(source, /function publicUrl|export function publicUrl/, "redirect helper should centralize public URL building");
});
