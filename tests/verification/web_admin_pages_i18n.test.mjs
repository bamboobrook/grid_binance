import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("admin pages localize on-call console copy through ui_lang instead of hardcoded mixed-language labels", () => {
  const pageExpectations = [
    {
      path: "apps/web/src/app/admin/dashboard/page.tsx",
      zh: /值班总览|运营总览|待处理充值|会员风险|审计事件/,
      en: /On-call Console|Operations Overview|Pending Deposits|Membership Risk|Audit Events/,
    },
    {
      path: "apps/web/src/app/admin/users/page.tsx",
      zh: /用户台账|用户管理|权限角色|注册状态/,
      en: /User Ledger|User Management|Role Boundary|Registration State/,
    },
    {
      path: "apps/web/src/app/admin/memberships/page.tsx",
      zh: /会员生命周期|价格矩阵|会员开通/,
      en: /Membership Lifecycle|Price Matrix|Open Membership/,
    },
    {
      path: "apps/web/src/app/admin/deposits/page.tsx",
      zh: /充值审核|人工入账|复核原因/,
      en: /Deposit Review|Manual Credit|Review Reason/,
    },
    {
      path: "apps/web/src/app/admin/address-pools/page.tsx",
      zh: /地址池治理|地址池压力|链路分配/,
      en: /Address Pool Governance|Pool Pressure|Chain Allocation/,
    },
    {
      path: "apps/web/src/app/admin/templates/page.tsx",
      zh: /模板治理|就绪门禁|模板清单/,
      en: /Template Governance|Readiness Gates|Template Inventory/,
    },
    {
      path: "apps/web/src/app/admin/strategies/page.tsx",
      zh: /策略监督|运行态|预检|选中详情/,
      en: /Strategy Supervision|Runtime State|Pre-flight|Selected Detail/,
    },
    {
      path: "apps/web/src/app/admin/sweeps/page.tsx",
      zh: /归集审批|金库地址|提交归集/,
      en: /Sweep Approval|Treasury Address|Submit Sweep/,
    },
    {
      path: "apps/web/src/app/admin/audit/page.tsx",
      zh: /审计留痕|会话摘要|变更前后/,
      en: /Audit Trail|Session Summary|Before \/ After/,
    },
    {
      path: "apps/web/src/app/admin/system/page.tsx",
      zh: /系统配置|确认数策略|变更影响/,
      en: /System Configuration|Confirmation Policy|Change Impact/,
    },
  ];

  for (const expectation of pageExpectations) {
    const source = read(expectation.path);
    assert.match(source, /cookies\(/, `${expectation.path} should read cookies for ui_lang`);
    assert.match(source, /UI_LANGUAGE_COOKIE/, `${expectation.path} should read the ui_lang cookie key`);
    assert.match(source, /resolveUiLanguage/, `${expectation.path} should resolve ui language`);
    assert.match(source, /pickText\(/, `${expectation.path} should localize copy through pickText`);
    assert.match(source, expectation.zh, `${expectation.path} should include Chinese on-call console copy`);
    assert.match(source, expectation.en, `${expectation.path} should include English on-call console copy`);
    assert.doesNotMatch(source, /"[^"]*[\u4e00-\u9fa5][^"]* \/ [^"]*[A-Za-z][^"]*"/, `${expectation.path} should not hardcode mixed Chinese/English labels`);
  }

  const usersPage = read("apps/web/src/app/admin/users/page.tsx");
  const membershipsPage = read("apps/web/src/app/admin/memberships/page.tsx");
  const depositsPage = read("apps/web/src/app/admin/deposits/page.tsx");
  const strategiesPage = read("apps/web/src/app/admin/strategies/page.tsx");

  assert.doesNotMatch(usersPage, /Commercial record only|Registered · verified|Registered · pending verification/, "users page should not leak raw English status copy");
  assert.doesNotMatch(membershipsPage, /Monthly, quarterly, and yearly pricing are managed here\./, "memberships page copy should not overstate plan coverage");
  assert.doesNotMatch(depositsPage, /MANUAL_CREDIT_MEMBERSHIP/, "deposits page should not expose magic confirmation strings to operators");
  assert.doesNotMatch(strategiesPage, /const focused = items\[0\] \?\? null;/, "strategies page should not always focus the first strategy");

  const billingRedirect = read("apps/web/src/app/admin/billing/page.tsx");
  assert.match(billingRedirect, /redirect\("\/admin\/deposits"\)/, "admin billing should remain a redirect to deposits");
});
