import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

const config = fs.readFileSync("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/deploy/nginx/default.conf", "utf8");

test("nginx forwards host metadata needed for Next route redirects", () => {
  assert.match(config, /proxy_set_header Host \$http_host;/);
  assert.match(config, /proxy_set_header X-Forwarded-Proto \$scheme;/);
  assert.match(config, /proxy_set_header X-Forwarded-Host \$http_host;/, "nginx should preserve the external host and port for Next route handlers");
  assert.match(config, /proxy_set_header X-Forwarded-Port \$server_port;/, "nginx should preserve the external port for Next route handlers");
});
