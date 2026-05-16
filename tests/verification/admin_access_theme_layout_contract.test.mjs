import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("admin access flow keeps the admin login context instead of bouncing to the user workspace", () => {
  const proxy = read("apps/web/proxy.ts");
  const adminLayout = read("apps/web/app/[locale]/admin/layout.tsx");
  const adminLoginRoute = read("apps/web/app/[locale]/admin/login/route.ts");

  assert.match(
    proxy,
    /pathname\s*===\s*"\/admin\/login"|pathname\.startsWith\("\/admin\/login"\)/,
    "proxy should explicitly allow the localized admin login entrypoint before protected-admin redirects",
  );
  assert.match(
    adminLayout,
    /redirect\("\/" \+ locale \+ "\/admin\/login\?error=" \+ error\)/,
    "admin layout should send unauthenticated admin visitors back to the admin login entrypoint",
  );
  assert.match(
    adminLoginRoute,
    /admin\/dashboard/,
    "admin login redirect should preserve the admin dashboard as the post-login target",
  );
});

test("shared shells and key entry surfaces stay theme-aware instead of forcing a dark palette", () => {
  const files = [
    "apps/web/components/shell/admin-shell.tsx",
    "apps/web/app/[locale]/(public)/login/page.tsx",
    "apps/web/app/[locale]/app/dashboard/page.tsx",
    "apps/web/components/strategies/strategy-inventory-table.tsx",
  ];
  const hardcodedDarkPalette =
    /bg-\[#(?:0a0e17|0f141f|111827|1f2937|1e293b)[^\"]*|text-white|text-slate-(?:200|300|400|500)|border-slate-(?:700|800)|border-indigo-900\/50|hover:bg-\[#/;

  for (const path of files) {
    assert.doesNotMatch(
      read(path),
      hardcodedDarkPalette,
      `${path} should rely on theme tokens so light mode can render correctly`,
    );
  }
});

test("user-shell quick stats remain in a single row instead of collapsing into two rows", () => {
  const globals = read("apps/web/styles/globals.css");

  assert.match(globals, /\.metric-strip\s*\{[^}]*flex-nowrap/s, "metric strip should stay on one row");
  assert.match(
    globals,
    /\.metric-strip__item\s*\{[^}]*min-w-\[[^\]]+\][^}]*flex-1/s,
    "metric items should have a minimum width but still share the same row cleanly",
  );
});
