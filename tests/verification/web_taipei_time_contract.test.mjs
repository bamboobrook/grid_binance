import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("user and admin pages route visible timestamps through the shared UTC+8 formatter", () => {
  const helper = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/lib/ui/time.ts");
  const files = [
    "apps/web/app/[locale]/app/dashboard/page.tsx",
    "apps/web/app/[locale]/app/notifications/page.tsx",
    "apps/web/app/[locale]/app/telegram/page.tsx",
    "apps/web/app/[locale]/app/orders/page.tsx",
    "apps/web/app/[locale]/app/analytics/page.tsx",
    "apps/web/app/[locale]/app/strategies/[id]/page.tsx",
    "apps/web/app/[locale]/app/billing/page.tsx",
    "apps/web/app/[locale]/admin/dashboard/page.tsx",
    "apps/web/app/[locale]/admin/audit/page.tsx",
  ];

  assert.match(helper, /Asia\/Taipei|UTC\+8/, "time helper should format timestamps in the UTC+8 timezone");
  for (const file of files) {
    const source = read(`/home/bumblebee/Project/grid_binance/.worktrees/full-v1/${file}`);
    assert.match(source, /formatTaipeiDateTime|formatTaipeiDate/, `${file} should use the shared UTC+8 formatter`);
    assert.doesNotMatch(source, /replace\("T", " "\)\.slice\(0, 16\)|slice\(0, 19\)\.replace\("T", " "\)/, `${file} should not hand-format timestamps anymore`);
  }
});
