import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("verify email page no longer blocks first login with legacy-only copy", () => {
  const page = read("apps/web/app/[locale]/(public)/verify-email/page.tsx");

  assert.doesNotMatch(
    page,
    /必须先完成邮箱验证|Verification must complete before login is allowed|首次登录前，请先去邮箱查看验证码|before your first login/i,
    "verify email page should stop telling newly registered users that login is blocked by email verification",
  );
  assert.match(
    page,
    /账号已可直接登录|直接去登录|旧验证码|Account ready|sign in now|legacy verification/i,
    "verify email page should explain that the page only remains for legacy verification flows",
  );
});

test("admin shell build path keeps per-endpoint fallbacks and a server-level shell fallback", () => {
  const adminState = read("apps/web/lib/api/admin-product-state.ts");
  const serverState = read("apps/web/lib/api/server.ts");

  assert.match(
    adminState,
    /tryFetchAdminJson<AdminMembershipList>\("\/admin\/memberships"\)/,
    "admin shell should degrade memberships fetches instead of throwing",
  );
  assert.match(
    adminState,
    /tryFetchAdminJson<AdminDepositsResponse>\(/,
    "admin shell should degrade deposits fetches instead of throwing",
  );
  assert.match(
    adminState,
    /degraded|降级|fallback/i,
    "admin shell should surface a degraded-shell fallback signal when supporting endpoints fail",
  );
  assert.match(
    serverState,
    /catch\s*\{\s*return buildFallbackAdminShellSnapshot\(lang\)/,
    "server snapshot loader should return a fallback admin shell instead of crashing",
  );
});

test("admin supporting pages read fallback-backed shared admin data instead of hard-throwing endpoint requests", () => {
  const adminState = read("apps/web/lib/api/admin-product-state.ts");

  for (const path of [
    '"/profile"',
    '"/admin/users"',
    '"/admin/memberships"',
    '"/admin/memberships/plans"',
    '"/admin/address-pools"',
    '"/admin/templates"',
    '"/admin/strategies"',
    '"/admin/sweeps"',
    '"/admin/audit"',
    '"/admin/system"',
  ]) {
    assert.match(
      adminState,
      new RegExp(`case ${path.replace(/[.*+?^${}()|[\\]\\\\]/g, "\\$&")}:`),
      `${path} should have an explicit admin fallback payload`,
    );
  }

  assert.match(
    adminState,
    /return fallbackAdminData\(path\) as T;/,
    "shared admin fetches should fall back to safe empty payloads when a supporting endpoint fails",
  );
  assert.match(
    adminState,
    /export async function getCurrentAdminProfile\(\)\s*\{\s*return fetchAdminJson<AdminProfile>\("\/profile"\);\s*\}/,
    "profile fetch should also flow through the fallback-backed fetch helper",
  );
});

test("verify email route is now explicitly legacy-only", () => {
  const route = read("apps/web/app/api/auth/verify-email/route.ts");
  const page = read("apps/web/app/[locale]/(public)/verify-email/page.tsx");

  assert.match(
    route,
    /LEGACY_VERIFY_NOTICE\s*=\s*"legacy-only"/,
    "verify-email route should mark its flow as legacy-only",
  );
  assert.match(
    route,
    /url\.searchParams\.set\("notice", LEGACY_VERIFY_NOTICE\)/,
    "verify-email route should carry a legacy-only notice when redirecting back to the verify page",
  );
  assert.match(
    route,
    /legacy verification request failed/i,
    "verify-email route should use a legacy-specific fallback error message",
  );
  assert.match(
    page,
    /notice === "legacy-only"|legacy compatibility entrypoint|旧验证码兼容入口/i,
    "verify-email page should render explicit legacy-only messaging",
  );
});
