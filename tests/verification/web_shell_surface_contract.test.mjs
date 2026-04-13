import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("shell surfaces keep bilingual navigation, styled shell classes, and admin login entrypoints", () => {
  const sidebar = read("apps/web/components/layout/sidebar.tsx");
  const topbar = read("apps/web/components/layout/topbar.tsx");
  const globals = read("apps/web/styles/globals.css");
  const notificationsPage = read("apps/web/app/[locale]/app/notifications/page.tsx");
  const userShell = read("apps/web/components/shell/user-shell.tsx");
  const mockData = read("apps/web/lib/api/mock-data.ts");

  assert.doesNotMatch(sidebar, /name:\s*'Analytics'|>Trading</, "user sidebar should not keep raw English-only navigation labels");
  assert.doesNotMatch(topbar, /Create Bot|Binance Spot|Connected/, "topbar should not keep hardcoded English operator copy");

  assert.match(globals, /\.shell-sidebar\b/, "globals should style shell sidebar surfaces");
  assert.match(globals, /\.shell-topbar\b/, "globals should style shell topbar surfaces");
  assert.match(globals, /\.metric-strip\b/, "globals should style metric strips");
  assert.match(globals, /\.text-list\b/, "globals should style shared text lists");
  assert.match(globals, /\.content-grid--metrics\b/, "globals should style metrics grid layouts");
  assert.match(globals, /\.ui-dialog\b/, "globals should style dialog frames");
  assert.match(globals, /\.shell-preferences\b/, "globals should style shell preferences controls");

  assert.match(notificationsPage, /params:\s*Promise<\{\s*locale:\s*string\s*\}>/, "notifications page should read locale params");
  assert.match(notificationsPage, /AppShellSection/, "notifications page should render inside the user shell surface");
  assert.match(userShell, /snapshot\.nav/, "user shell should render navigation from the live snapshot");
  assert.match(mockData, /\/app\/notifications/, "user navigation data should expose the in-app notifications route");

  const hasAdminLoginPage = fs.existsSync("apps/web/app/[locale]/admin/login/page.tsx");
  const hasAdminLoginRoute = fs.existsSync("apps/web/app/admin/login/route.ts");
  assert.ok(hasAdminLoginPage || hasAdminLoginRoute, "admin login entrypoint should exist so /admin/login is not a 404");
});
