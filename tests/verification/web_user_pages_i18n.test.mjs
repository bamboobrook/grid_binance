import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

const localizedPages = [
  {
    path: "apps/web/src/app/app/dashboard/page.tsx",
    patterns: [/用户总览|风险|驾驶舱/, /Dashboard|risk|cockpit|overview/i],
  },
  {
    path: "apps/web/src/app/app/strategies/page.tsx",
    patterns: [/策略目录|批量操作|筛选/, /Strategies|batch|filter|toolbar/i],
  },
  {
    path: "apps/web/src/app/app/strategies/new/page.tsx",
    patterns: [/新建策略|草稿|模板/, /New Strategy|draft|template/i],
  },
  {
    path: "apps/web/src/app/app/orders/page.tsx",
    patterns: [/订单|成交|交易记录/, /Orders|fills|history/i],
  },
  {
    path: "apps/web/src/app/app/analytics/page.tsx",
    patterns: [/分析|统计|资金快照/, /Analytics|statistics|wallet/i],
  },
  {
    path: "apps/web/src/app/app/security/page.tsx",
    patterns: [/安全|密码|双重验证|TOTP/, /Security|password|TOTP/i],
  },
  {
    path: "apps/web/src/app/app/telegram/page.tsx",
    patterns: [/绑定|通知|机器人/, /Telegram|notification|bot/i],
  },
  {
    path: "apps/web/src/app/app/help/page.tsx",
    patterns: [/帮助中心|指南|文档/, /Help Center|guide|docs/i],
  },
];

test("user-facing SaaS pages localize via ui_lang without rendering both languages at once", () => {
  for (const page of localizedPages) {
    const source = read(page.path);
    assert.match(source, /pickText|resolveUiLanguage|UI_LANGUAGE_COOKIE/, `${page.path} should use shared language preference helpers`);
    assert.doesNotMatch(source, /pickText\(lang,\s*zh,\s*en\)\s*\+\s*" \/ "\s*\+\s*pickText\(lang,\s*en,\s*zh\)/, `${page.path} should not render Chinese and English together`);

    for (const pattern of page.patterns) {
      assert.match(source, pattern, `${page.path} should keep both language resources in source`);
    }
  }
});

test("billing, exchange, and strategy detail follow the new product constraints", () => {
  const billingPage = read("apps/web/src/app/app/billing/page.tsx");
  const exchangePage = read("apps/web/src/app/app/exchange/page.tsx");
  const detailPage = read("apps/web/src/app/app/strategies/[id]/page.tsx");

  assert.match(billingPage, /plans\.map|selectedPlan|selectedPrice/, "billing page should derive plan and price choices from overview.plans");
  assert.doesNotMatch(billingPage, /<option value="monthly">Monthly<\/option>/, "billing page should not hardcode monthly option markup");
  assert.doesNotMatch(billingPage, /<option value="quarterly">Quarterly<\/option>/, "billing page should not hardcode quarterly option markup");
  assert.doesNotMatch(billingPage, /<option value="yearly">Yearly<\/option>/, "billing page should not hardcode yearly option markup");

  assert.match(exchangePage, /未绑定|Not connected yet|未测试|Awaiting validation|校验失败|Validation failed/, "exchange page should surface unbound, untested, and failed validation states");
  assert.match(detailPage, /describeStrategyStatus|statusCopy|statusMeta/, "strategy detail should map backend states to user-facing copy");
  assert.doesNotMatch(detailPage, /strategy\.status\.replaceAll/, "strategy detail should not render raw backend status enums");
});

test("legacy user route aliases remain redirects instead of rendering separate shells", () => {
  const membershipPage = read("apps/web/src/app/app/membership/page.tsx");
  const notificationsPage = read("apps/web/src/app/app/notifications/page.tsx");

  assert.match(membershipPage, /redirect\("\/app\/billing"\)/);
  assert.match(notificationsPage, /redirect\("\/app\/telegram"\)/);
  assert.doesNotMatch(membershipPage, /AppShellSection|StatusBanner|Card/);
  assert.doesNotMatch(notificationsPage, /AppShellSection|StatusBanner|Card/);
});
