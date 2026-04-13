import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const config = fs.readFileSync("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/deploy/nginx/default.conf", "utf8");

test("nginx keeps api health on the Rust backend but sends web proxy routes to Next", () => {
  assert.match(config, /location = \/api\/healthz \{[\s\S]*proxy_pass http:\/\/\$api_upstream;/, "api health should stay on the Rust backend");
  assert.match(config, /location \/api\/ \{[\s\S]*proxy_pass http:\/\/\$web_upstream;/, "web auth and user form proxy routes should go to Next");
  assert.doesNotMatch(config, /location \/api\/ \{[\s\S]*rewrite \^\/api\/\(\.\*\)\$ \/\$1 break;[\s\S]*proxy_pass http:\/\/\$api_upstream;/, "the generic /api route must not rewrite everything to the Rust backend");
});
