import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

function read(path) {
  return readFileSync(path, "utf8");
}

test("user-facing business copy routes through localization helpers instead of raw backend strings", () => {
  const notifications = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/notifications/page.tsx");
  const appLayout = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/layout.tsx");
  const billing = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/billing/page.tsx");
  const server = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/lib/api/server.ts");
  const strategyDetail = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/[id]/page.tsx");

  assert.match(notifications, /localizeNotificationTitle/);
  assert.match(appLayout, /localizeNotificationMessage/);
  assert.match(appLayout, /localizeNotificationTitle/);
  assert.match(billing, /describeMembershipStatus/);
  assert.match(server, /describeMembershipStatus/);
  assert.match(strategyDetail, /describeRuntimeEventDetail/);
});

test("light theme risk surfaces keep readable contrast tokens", () => {
  const banner = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/ui/status-banner.tsx");
  const chip = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/ui/chip.tsx");
  const strategyForm = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/strategies/strategy-workspace-form.tsx");

  assert.doesNotMatch(banner, /text-blue-400|text-emerald-400|text-amber-400|text-red-400/);
  assert.doesNotMatch(chip, /text-blue-400|text-emerald-400|text-amber-400|text-red-400/);
  assert.doesNotMatch(strategyForm, /text-amber-200/);
});

test("admin memberships desk no longer hard-locks nowrap mono table layout", () => {
  const memberships = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/admin/memberships/page.tsx");
  const table = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/ui/table.tsx");

  assert.doesNotMatch(memberships, /whitespace-nowrap/);
  assert.doesNotMatch(table, /font-mono/);
  assert.match(memberships, /min-w-\[220px\]/);
});
