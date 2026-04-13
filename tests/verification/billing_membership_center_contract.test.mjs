import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("billing page is presented as a membership center with a client-side linked order form", () => {
  const page = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/billing/page.tsx");
  const form = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/components/billing/membership-order-form.tsx");
  const nav = read("/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/lib/api/mock-data.ts");

  assert.match(page, /会员中心|Membership Center/, "billing page should be renamed to membership center");
  assert.doesNotMatch(page, /content-grid--metrics/, "membership center should not render the old three-box pricing grid");
  assert.match(form, /按月支付|Pay monthly/, "membership center should summarize monthly pricing as a simple line item");
  assert.match(form, /按季度支付|Pay quarterly/, "membership center should summarize quarterly pricing as a simple line item");
  assert.match(form, /按年支付|Pay yearly/, "membership center should summarize yearly pricing as a simple line item");

  assert.match(form, /"use client"|'use client'/, "membership order form should be a client component so pricing can react immediately");
  assert.match(form, /useState\(|useMemo\(/, "membership order form should keep local selection state for live pricing updates");
  assert.match(form, /按月支付|Pay monthly/, "plan select should translate monthly");
  assert.match(form, /按季度支付|Pay quarterly/, "plan select should translate quarterly");
  assert.match(form, /按年支付|Pay yearly/, "plan select should translate yearly");
  assert.match(form, /当前选择价格|Selected price/, "membership order form should surface the current linked price");
  assert.match(form, /onChange=|setSelectedPlan|setSelectedChain|setSelectedToken/, "membership order form should update the displayed price when plan or market inputs change");

  assert.doesNotMatch(page, /import\s*\{\s*MembershipOrderForm\s*,\s*describePlanSummary\s*\}\s*from\s*"@\/components\/billing\/membership-order-form"/, "membership center server page must not import client-side helpers for direct execution");
  assert.match(nav, /会员中心|Membership Center/, "user navigation should point to membership center instead of generic billing wording");
});
