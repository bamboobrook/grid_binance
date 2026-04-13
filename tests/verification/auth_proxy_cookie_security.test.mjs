import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("auth proxy routes derive secure-cookie behavior from request scheme instead of NODE_ENV", () => {
  const authHelpers = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/lib/auth.ts");
  const adminBootstrap = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/auth/admin-bootstrap/route.ts");
  const registerRoute = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/auth/register/route.ts");
  const resetRoute = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/api/auth/password-reset/route.ts");

  assert.match(authHelpers, /x-forwarded-proto/i, "auth helpers should inspect forwarded proto");
  assert.match(authHelpers, /shouldUseSecureCookie/, "auth helpers should expose a shared secure-cookie helper");
  assert.match(adminBootstrap, /shouldUseSecureCookie\(request\)/, "admin bootstrap should use the shared secure-cookie helper");
  assert.match(resetRoute, /shouldUseSecureCookie\(request\)/, "password reset route should use the shared secure-cookie helper");
  assert.doesNotMatch(adminBootstrap, /NODE_ENV === "production"/, "admin bootstrap should not rely only on NODE_ENV for secure cookies");
  assert.doesNotMatch(resetRoute, /NODE_ENV === "production"/, "password reset route should not rely only on NODE_ENV for secure cookies");
  assert.doesNotMatch(registerRoute, /pending_verify_code|cookies\.set\(/, "register route should not leak legacy verification cookies after direct-sign-in registration");
});
