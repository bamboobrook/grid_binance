import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function escapePattern(input) {
  return input.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function loadTsExports(path, exportNames) {
  const source = read(path)
    .replace(/\sas const/g, "")
    .replace(/\sas \(typeof VALID_HELP_ARTICLES\)\[number\]/g, "")
    .replace(/\?:/g, ":")
    .replace(/: string \| string\[\]/g, "")
    .replace(/: string \| null/g, "")
    .replace(/: string/g, "")
    .replace(/: boolean/g, "")
    .replace(/export const /g, "const ")
    .replace(/export function /g, "function ");

  return new Function(`${source}\nreturn { ${exportNames.join(", ")} };`)();
}

test("web app shell structure aligns with shared public, user, and admin route systems", () => {
  const requiredFiles = [
    "apps/web/src/app/(public)/page.tsx",
    "apps/web/src/app/(public)/layout.tsx",
    "apps/web/src/app/app/layout.tsx",
    "apps/web/src/app/admin/layout.tsx",
    "apps/web/src/app/app/strategies/new/page.tsx",
    "apps/web/src/app/app/orders/page.tsx",
    "apps/web/src/app/app/telegram/page.tsx",
    "apps/web/src/app/app/help/page.tsx",
    "apps/web/src/app/admin/memberships/page.tsx",
    "apps/web/src/app/admin/deposits/page.tsx",
    "apps/web/src/app/admin/strategies/page.tsx",
    "apps/web/src/app/admin/sweeps/page.tsx",
    "apps/web/src/app/admin/system/page.tsx",
    "apps/web/src/components/shell/public-shell.tsx",
    "apps/web/src/components/shell/user-shell.tsx",
    "apps/web/src/components/shell/admin-shell.tsx",
    "apps/web/src/components/shell/path-utils.ts",
    "apps/web/src/components/ui/status-banner.tsx",
    "apps/web/src/components/ui/card.tsx",
    "apps/web/src/components/ui/table.tsx",
    "apps/web/src/components/ui/form.tsx",
    "apps/web/src/components/ui/tabs.tsx",
    "apps/web/src/components/ui/chip.tsx",
    "apps/web/src/components/ui/dialog.tsx",
    "apps/web/src/lib/api/server.ts",
    "apps/web/src/lib/api/mock-data.ts",
    "apps/web/src/lib/api/help-articles.ts",
  ];

  for (const file of requiredFiles) {
    assert.ok(fs.existsSync(file), `${file} should exist`);
  }

  assert.equal(fs.existsSync("apps/web/src/app/page.tsx"), false, "homepage should be owned by /(public)/page.tsx, not app/page.tsx");

  const homePage = read("apps/web/src/app/(public)/page.tsx");
  const publicLayout = read("apps/web/src/app/(public)/layout.tsx");
  const userLayout = read("apps/web/src/app/app/layout.tsx");
  const adminLayout = read("apps/web/src/app/admin/layout.tsx");
  const publicShell = read("apps/web/src/components/shell/public-shell.tsx");
  const userShell = read("apps/web/src/components/shell/user-shell.tsx");
  const adminShell = read("apps/web/src/components/shell/admin-shell.tsx");
  const mockData = read("apps/web/src/lib/api/mock-data.ts");
  const helpPage = read("apps/web/src/app/app/help/page.tsx");
  const helpSlugPage = read("apps/web/src/app/help/[slug]/page.tsx");

  assert.doesNotMatch(homePage, /PublicShell/, "homepage content should be wrapped by the public layout instead of re-rendering PublicShell");
  assert.doesNotMatch(homePage, /<main[\s>]/, "homepage should rely on shared public shell layout");
  assert.match(publicLayout, /PublicShell/);
  assert.doesNotMatch(userLayout, /\/app\/dashboard/, "user layout must not hardcode dashboard as active state");
  assert.doesNotMatch(adminLayout, /\/admin\/dashboard/, "admin layout must not hardcode dashboard as active state");

  assert.match(publicShell, /usePathname/);
  assert.match(userShell, /usePathname/);
  assert.match(adminShell, /usePathname/);

  const legacyRedirects = [
    ["apps/web/src/app/app/analytics/page.tsx", /redirect\("\/app\/orders"\)/],
    ["apps/web/src/app/app/membership/page.tsx", /redirect\("\/app\/billing"\)/],
    ["apps/web/src/app/app/notifications/page.tsx", /redirect\("\/app\/telegram"\)/],
    ["apps/web/src/app/admin/billing/page.tsx", /redirect\("\/admin\/deposits"\)/],
  ];

  for (const [page, pattern] of legacyRedirects) {
    const source = read(page);
    assert.match(source, pattern, `${page} should redirect to the documented route`);
    assert.doesNotMatch(source, /<main[\s>]/, `${page} should not render route-local markup`);
  }

  const routePages = [
    "apps/web/src/app/(public)/login/page.tsx",
    "apps/web/src/app/(public)/register/page.tsx",
    "apps/web/src/app/(public)/page.tsx",
    "apps/web/src/app/app/dashboard/page.tsx",
    "apps/web/src/app/app/exchange/page.tsx",
    "apps/web/src/app/app/strategies/page.tsx",
    "apps/web/src/app/app/strategies/new/page.tsx",
    "apps/web/src/app/app/strategies/[id]/page.tsx",
    "apps/web/src/app/app/orders/page.tsx",
    "apps/web/src/app/app/billing/page.tsx",
    "apps/web/src/app/app/telegram/page.tsx",
    "apps/web/src/app/app/security/page.tsx",
    "apps/web/src/app/app/help/page.tsx",
    "apps/web/src/app/admin/dashboard/page.tsx",
    "apps/web/src/app/admin/users/page.tsx",
    "apps/web/src/app/admin/memberships/page.tsx",
    "apps/web/src/app/admin/deposits/page.tsx",
    "apps/web/src/app/admin/address-pools/page.tsx",
    "apps/web/src/app/admin/templates/page.tsx",
    "apps/web/src/app/admin/strategies/page.tsx",
    "apps/web/src/app/admin/sweeps/page.tsx",
    "apps/web/src/app/admin/audit/page.tsx",
    "apps/web/src/app/admin/system/page.tsx",
  ];

  for (const page of routePages) {
    assert.doesNotMatch(read(page), /<main[\s>]/, `${page} should rely on shared shell layout`);
  }

  for (const href of [
    "/app/dashboard",
    "/app/exchange",
    "/app/strategies",
    "/app/strategies/new",
    "/app/orders",
    "/app/billing",
    "/app/telegram",
    "/app/security",
    "/app/help",
    "/admin/dashboard",
    "/admin/users",
    "/admin/memberships",
    "/admin/deposits",
    "/admin/address-pools",
    "/admin/templates",
    "/admin/strategies",
    "/admin/sweeps",
    "/admin/audit",
    "/admin/system",
  ]) {
    assert.match(mockData, new RegExp(escapePattern(href)));
  }

  for (const legacyHref of ["/app/analytics", "/app/membership", "/app/notifications", "/admin/billing"]) {
    assert.doesNotMatch(mockData, new RegExp(escapePattern(legacyHref)));
  }

  assert.match(helpPage, /normalizeHelpArticle/);
  assert.match(helpPage, /notFound\(/);
  assert.match(helpSlugPage, /getHelpArticle/);
  assert.match(helpSlugPage, /notFound\(/);
  assert.match(helpSlugPage, /shell--public/);
  assert.match(helpSlugPage, /\/app\/help\?article=\$\{article\.slug\}/);

  const serverApi = read("apps/web/src/lib/api/server.ts");
  assert.match(serverApi, /getUserDashboardSnapshot/);
  assert.match(serverApi, /getAdminDashboardSnapshot/);
  assert.match(serverApi, /server-only|"use server"/);
});

test("shell and help helpers enforce route behavior", async () => {
  const { isNavHrefActive } = await import("../../apps/web/src/components/shell/path-utils.ts");
  const { isValidHelpArticle, normalizeHelpArticle } = await import("../../apps/web/src/lib/api/help-articles.ts");

  assert.equal(isNavHrefActive("/app/orders", "/app/orders"), true);
  assert.equal(isNavHrefActive("/app/strategies/grid-btc", "/app/strategies"), true);
  assert.equal(isNavHrefActive("/app/telegram", "/app/orders"), false);

  assert.equal(isValidHelpArticle("expiry-reminder"), true);
  assert.equal(isValidHelpArticle("unknown-slug"), false);
  assert.equal(normalizeHelpArticle("expiry-reminder"), "expiry-reminder");
  assert.equal(normalizeHelpArticle(["expiry-reminder", "other"]), "expiry-reminder");
  assert.equal(normalizeHelpArticle("unknown-slug"), null);
});
