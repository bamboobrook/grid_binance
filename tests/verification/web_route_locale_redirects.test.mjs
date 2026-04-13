import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("web auth and action redirects preserve locale-aware routes instead of bare app/admin paths", () => {
  const auth = read("apps/web/lib/auth.ts");
  const adminBootstrap = read("apps/web/app/api/auth/admin-bootstrap/route.ts");
  const adminShared = read("apps/web/app/api/admin/_shared.ts");
  const userBilling = read("apps/web/app/api/user/billing/route.ts");
  const userExchange = read("apps/web/app/api/user/exchange/route.ts");
  const userSecurity = read("apps/web/app/api/user/security/route.ts");
  const userTelegram = read("apps/web/app/api/user/telegram/route.ts");
  const userStrategyBatch = read("apps/web/app/api/user/strategies/batch/route.ts");
  const userStrategyDetail = read("apps/web/app/api/user/strategies/[id]/route.ts");
  const userStrategyCreate = read("apps/web/app/api/user/strategies/create/route.ts");

  assert.match(auth, /function requestLocale\(/, "auth helpers should derive locale from request context");
  assert.match(auth, /function localizedPath\(/, "auth helpers should centralize locale-aware route building");
  assert.match(auth, /localizedAppPath\(/, "auth helpers should expose localized app targets");
  assert.match(auth, /localizedAdminPath\(/, "auth helpers should expose localized admin targets");
  assert.match(adminBootstrap, /localizedPublicPath\(|publicUrl\(/, "admin bootstrap should reuse locale-aware public redirect helpers");
  assert.doesNotMatch(auth, /safeRedirectTarget\(nextPath,\s*"\/app\/dashboard"\)/, "login should not default to a bare /app/dashboard target");
  assert.doesNotMatch(auth, /const url = publicUrl\(request, pathname\)/, "public auth error redirects should not drop the active locale");

  for (const [name, source] of [
    ["admin shared", adminShared],
    ["user billing", userBilling],
    ["user exchange", userExchange],
    ["user security", userSecurity],
    ["user telegram", userTelegram],
    ["user strategy batch", userStrategyBatch],
    ["user strategy detail", userStrategyDetail],
    ["user strategy create", userStrategyCreate],
  ]) {
    assert.doesNotMatch(source, /publicUrl\(request,\s*[`"]\/(app|admin|login|register|password-reset)/, `${name} should not redirect to bare locale-less routes`);
  }

  assert.match(adminShared, /localizedAdminPath\(|localizedPublicPath\(|requestLocale\(/, "admin redirect helpers should reuse locale-aware auth helpers");
});
