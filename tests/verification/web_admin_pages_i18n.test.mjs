import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

test("admin pages localize on-call console copy through the route locale instead of cookie-only mixed-language rendering", () => {
  const pageExpectations = [
    {
      path: "apps/web/app/[locale]/admin/dashboard/page.tsx",
      zh: /值班总览|运营总览|待处理充值|会员风险|审计事件/,
      en: /On-call Console|Operations Overview|Pending Deposits|Membership Risk|Audit Events/,
    },
    {
      path: "apps/web/app/[locale]/admin/users/page.tsx",
      zh: /用户台账|用户管理|权限角色|注册状态/,
      en: /User Ledger|User Management|Role Boundary|Registration State/,
    },
    {
      path: "apps/web/app/[locale]/admin/memberships/page.tsx",
      zh: /会员生命周期|价格矩阵|会员开通/,
      en: /Membership Lifecycle|Price Matrix|Open Membership/,
    },
    {
      path: "apps/web/app/[locale]/admin/deposits/page.tsx",
      zh: /充值审核|人工入账|复核原因/,
      en: /Deposit Review|Manual Credit|Review Reason/,
    },
    {
      path: "apps/web/app/[locale]/admin/address-pools/page.tsx",
      zh: /地址池治理|地址池压力|链路分配/,
      en: /Address Pool Governance|Pool Pressure|Chain Allocation/,
    },
    {
      path: "apps/web/app/[locale]/admin/templates/page.tsx",
      zh: /模板治理|就绪门禁|模板清单/,
      en: /Template Governance|Readiness Gates|Template Inventory/,
    },
    {
      path: "apps/web/app/[locale]/admin/strategies/page.tsx",
      zh: /策略监督|运行态|预检|选中详情/,
      en: /Strategy Supervision|Runtime State|Pre-flight|Selected Detail/,
    },
    {
      path: "apps/web/app/[locale]/admin/sweeps/page.tsx",
      zh: /归集审批|金库地址|提交归集/,
      en: /Sweep Approval|Treasury Address|Submit Sweep/,
    },
    {
      path: "apps/web/app/[locale]/admin/audit/page.tsx",
      zh: /审计留痕|会话摘要|变更前后/,
      en: /Audit Trail|Session Summary|Before \/ After/,
    },
    {
      path: "apps/web/app/[locale]/admin/system/page.tsx",
      zh: /系统配置|确认数策略|变更影响/,
      en: /System Configuration|Confirmation Policy|Change Impact/,
    },
  ];

  for (const expectation of pageExpectations) {
    const source = read(expectation.path);
    assert.match(source, /params:\s*Promise<\{\s*locale:\s*string\s*\}>/, `${expectation.path} should read locale params from the route`);
    assert.match(source, /resolveUiLanguageFromRoute\(/, `${expectation.path} should resolve language from locale route with cookie fallback`);
    assert.match(source, /pickText\(/, `${expectation.path} should localize copy through pickText`);
    assert.match(source, expectation.zh, `${expectation.path} should include Chinese admin copy`);
    assert.match(source, expectation.en, `${expectation.path} should include English admin copy`);
    assert.doesNotMatch(source, /"[^"]*[\u4e00-\u9fa5][^"]* \/ [^"]*[A-Za-z][^"]*"/, `${expectation.path} should not hardcode mixed Chinese/English labels`);
  }

  const dashboardPage = read("apps/web/app/[locale]/admin/dashboard/page.tsx");
  const usersPage = read("apps/web/app/[locale]/admin/users/page.tsx");
  const membershipsPage = read("apps/web/app/[locale]/admin/memberships/page.tsx");
  const depositsPage = read("apps/web/app/[locale]/admin/deposits/page.tsx");
  const strategiesPage = read("apps/web/app/[locale]/admin/strategies/page.tsx");
  const adminLayout = read("apps/web/app/[locale]/admin/layout.tsx");
  const adminShellState = read("apps/web/lib/api/admin-product-state.ts");

  assert.doesNotMatch(dashboardPage, /值班工作板|On-call Workboard/, "admin dashboard should use direct management panels instead of the old workboard copy");
  assert.doesNotMatch(dashboardPage, /\{\s*key:\s*"route"|route:\s*"\/admin/, "admin dashboard should not expose raw admin route columns in operator UI");
  assert.match(dashboardPage, /立即处理|查看详情|Open panel|Review now/, "admin dashboard should expose clickable action labels instead of route text");
  assert.doesNotMatch(usersPage, /Commercial record only|Registered · verified|Registered · pending verification/, "users page should not leak raw English status copy");
  assert.doesNotMatch(membershipsPage, /Monthly, quarterly, and yearly pricing are managed here\./, "memberships page copy should not overstate plan coverage");
  assert.doesNotMatch(depositsPage, /MANUAL_CREDIT_MEMBERSHIP/, "deposits page should not expose magic confirmation strings to operators");
  assert.doesNotMatch(strategiesPage, /const focused = items\[0\] \?\? null;/, "strategies page should not always focus the first strategy");
  assert.match(adminLayout, /params:\s*Promise<\{\s*locale:\s*string\s*\}>/, "admin layout should receive locale params");
  assert.match(adminLayout, /getAdminShellSnapshot\(locale\)/, "admin layout should build shell snapshot from the route locale");
  assert.match(adminShellState, /超级管理员|操作员|待验证/, "admin shell state should localize admin role copy");
  assert.match(adminShellState, /Super Admin|Operator Admin|Pending Verification/, "admin shell state should keep English role copy");

  const billingRedirect = read("apps/web/app/[locale]/admin/billing/page.tsx");
  assert.match(billingRedirect, /redirect\(`\/\$\{locale\}\/admin\/deposits`\)/, "admin billing should preserve the active locale when redirecting to deposits");
});
