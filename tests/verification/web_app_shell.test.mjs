import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("web app route groups use shared shells and ui system", () => {
  const requiredFiles = [
    "apps/web/src/app/(public)/layout.tsx",
    "apps/web/src/app/app/layout.tsx",
    "apps/web/src/app/admin/layout.tsx",
    "apps/web/src/components/shell/public-shell.tsx",
    "apps/web/src/components/shell/user-shell.tsx",
    "apps/web/src/components/shell/admin-shell.tsx",
    "apps/web/src/components/ui/status-banner.tsx",
    "apps/web/src/components/ui/card.tsx",
    "apps/web/src/components/ui/table.tsx",
    "apps/web/src/components/ui/form.tsx",
    "apps/web/src/components/ui/tabs.tsx",
    "apps/web/src/components/ui/chip.tsx",
    "apps/web/src/components/ui/dialog.tsx",
    "apps/web/src/lib/api/server.ts",
    "apps/web/src/lib/api/mock-data.ts"
  ];

  for (const path of requiredFiles) {
    assert.ok(fs.existsSync(path), `${path} should exist`);
  }

  const publicLayout = read("apps/web/src/app/(public)/layout.tsx");
  const userLayout = read("apps/web/src/app/app/layout.tsx");
  const adminLayout = read("apps/web/src/app/admin/layout.tsx");

  assert.match(publicLayout, /PublicShell/);
  assert.match(userLayout, /UserShell/);
  assert.match(adminLayout, /AdminShell/);

  const routePages = [
    "apps/web/src/app/(public)/login/page.tsx",
    "apps/web/src/app/(public)/register/page.tsx",
    "apps/web/src/app/app/dashboard/page.tsx",
    "apps/web/src/app/app/exchange/page.tsx",
    "apps/web/src/app/app/strategies/page.tsx",
    "apps/web/src/app/app/strategies/[id]/page.tsx",
    "apps/web/src/app/app/billing/page.tsx",
    "apps/web/src/app/app/analytics/page.tsx",
    "apps/web/src/app/app/security/page.tsx",
    "apps/web/src/app/app/membership/page.tsx",
    "apps/web/src/app/app/notifications/page.tsx",
    "apps/web/src/app/admin/dashboard/page.tsx",
    "apps/web/src/app/admin/users/page.tsx",
    "apps/web/src/app/admin/address-pools/page.tsx",
    "apps/web/src/app/admin/templates/page.tsx",
    "apps/web/src/app/admin/billing/page.tsx",
    "apps/web/src/app/admin/audit/page.tsx"
  ];

  for (const page of routePages) {
    const source = read(page);
    assert.doesNotMatch(source, /<main[\s>]/, `${page} should rely on shared shell layout`);
  }

  assert.match(read("apps/web/src/app/app/dashboard/page.tsx"), /Card|StatusBanner|AppShellSection/);
  assert.match(read("apps/web/src/app/admin/dashboard/page.tsx"), /Card|StatusBanner|AppShellSection/);
  assert.match(read("apps/web/src/app/(public)/register/page.tsx"), /Field|Form/);

  const serverApi = read("apps/web/src/lib/api/server.ts");
  assert.match(serverApi, /getUserDashboardSnapshot/);
  assert.match(serverApi, /getAdminDashboardSnapshot/);
  assert.match(serverApi, /server-only|"use server"/);
});
